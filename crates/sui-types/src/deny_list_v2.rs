// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::base_types::{EpochId, ObjectID, SuiAddress};
use crate::config::{Config, Setting};
use crate::deny_list_v1::{
    input_object_coin_types_for_denylist_check, DENY_LIST_COIN_TYPE_INDEX, DENY_LIST_MODULE,
};
use crate::dynamic_field::{get_dynamic_field_from_store, DOFWrapper};
use crate::error::{ExecutionError, ExecutionErrorKind, UserInputError, UserInputResult};
use crate::id::UID;
use crate::object::Object;
use crate::storage::{DenyListResult, ObjectStore};
use crate::transaction::{CheckedInputObjects, ReceivingObjects};
use crate::{MoveTypeTagTrait, SUI_DENY_LIST_OBJECT_ID, SUI_FRAMEWORK_PACKAGE_ID};
use move_core_types::ident_str;
use move_core_types::language_storage::{StructTag, TypeTag};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// esitmate of the size of the objects read for a deny list check
pub const CONFIG_READ_BYTES_SIZE_ESTIMATE: usize = {
    let address_size = 32usize;
    let id_size = address_size;
    let uid_size = id_size;
    let type_name_size = address_size;
    let bool_size = 1usize;
    let u64_size = 8usize;
    let option_some_bool_size = bool_size + 1usize;

    let config_key_size = u64_size + type_name_size;
    let address_key_size = address_size;

    let dof_field_size = uid_size + config_key_size + id_size;

    let config_size = uid_size;

    let setting_data_size = u64_size + option_some_bool_size + option_some_bool_size;
    let option_some_setting_data_size = setting_data_size + 1;
    let setting_size = option_some_setting_data_size;
    let setting_field_size = uid_size + address_key_size + setting_size;

    dof_field_size + config_size + setting_field_size
};

/// Rust representation of the Move type 0x2::coin::DenyCapV2.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DenyCapV2 {
    pub id: UID,
    pub allow_global_pause: bool,
}

/// Rust representation of the Move type 0x2::deny_list::ConfigKey.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ConfigKey {
    per_type_index: u64,
    per_type_key: Vec<u8>,
}

impl ConfigKey {
    pub fn type_() -> StructTag {
        StructTag {
            address: SUI_FRAMEWORK_PACKAGE_ID.into(),
            module: DENY_LIST_MODULE.to_owned(),
            name: ident_str!("ConfigKey").to_owned(),
            type_params: vec![],
        }
    }
}

impl MoveTypeTagTrait for ConfigKey {
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(Self::type_()))
    }
}

/// Rust representation of the Move type 0x2::deny_list::AddressKey.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct AddressKey(SuiAddress);

impl AddressKey {
    pub fn type_() -> StructTag {
        StructTag {
            address: SUI_FRAMEWORK_PACKAGE_ID.into(),
            module: DENY_LIST_MODULE.to_owned(),
            name: ident_str!("AddressKey").to_owned(),
            type_params: vec![],
        }
    }
}

impl MoveTypeTagTrait for AddressKey {
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(Self::type_()))
    }
}

/// Rust representation of the Move type 0x2::deny_list::GlobalPauseKey.
/// There is no u8 in the Move definition, however empty structs in Move
/// are represented as a single byte 0 in the serialized data.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct GlobalPauseKey(bool);

impl GlobalPauseKey {
    pub fn new() -> Self {
        Self(false)
    }
    pub fn type_() -> StructTag {
        StructTag {
            address: SUI_FRAMEWORK_PACKAGE_ID.into(),
            module: DENY_LIST_MODULE.to_owned(),
            name: ident_str!("GlobalPauseKey").to_owned(),
            type_params: vec![],
        }
    }
}

impl MoveTypeTagTrait for GlobalPauseKey {
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(Self::type_()))
    }
}

pub fn check_coin_deny_list_v2_during_signing(
    address: SuiAddress,
    input_objects: &CheckedInputObjects,
    receiving_objects: &ReceivingObjects,
    object_store: &dyn ObjectStore,
) -> UserInputResult {
    let coin_types = input_object_coin_types_for_denylist_check(input_objects, receiving_objects);
    for coin_type in coin_types {
        let Some(deny_list) = get_per_type_coin_deny_list_v2(&coin_type, object_store) else {
            return Ok(());
        };
        if check_global_pause(&deny_list, object_store, None) {
            return Err(UserInputError::CoinTypeGlobalPause { coin_type });
        }
        if check_address_denied_by_config(&deny_list, address, object_store, None) {
            return Err(UserInputError::AddressDeniedForCoin { address, coin_type });
        }
    }
    Ok(())
}

/// Returns 1) whether the coin deny list check passed,
///         2) whether the deny list object was checked at all, and
///         3) the number of regulated coin types checked.
pub fn check_coin_deny_list_v2_during_execution(
    written_objects: &BTreeMap<ObjectID, Object>,
    cur_epoch: EpochId,
    object_store: &dyn ObjectStore,
) -> DenyListResult {
    let mut new_coin_owners = BTreeMap::new();
    for obj in written_objects.values() {
        if obj.is_gas_coin() {
            continue;
        }
        let Some(coin_type) = obj.coin_type_maybe() else {
            continue;
        };
        let Ok(owner) = obj.owner.get_address_owner_address() else {
            continue;
        };
        new_coin_owners
            .entry(coin_type.to_canonical_string(false))
            .or_insert_with(BTreeSet::new)
            .insert(owner);
    }
    let deny_list_checked = !new_coin_owners.is_empty();
    let new_regulated_coin_owners = new_coin_owners
        .into_iter()
        .filter_map(|(coin_type, owners)| {
            let deny_list_config = get_per_type_coin_deny_list_v2(&coin_type, object_store)?;
            Some((coin_type, (deny_list_config, owners)))
        })
        .collect::<BTreeMap<_, _>>();
    let regulated_coin_types_checked = new_regulated_coin_owners.len() as u64;
    let result =
        check_new_regulated_coin_owners(new_regulated_coin_owners, cur_epoch, object_store);
    DenyListResult {
        result,
        deny_list_checked,
        regulated_coin_types_checked,
    }
}

fn check_new_regulated_coin_owners(
    new_regulated_coin_owners: BTreeMap<String, (Config, BTreeSet<SuiAddress>)>,
    cur_epoch: EpochId,
    object_store: &dyn ObjectStore,
) -> Result<(), ExecutionError> {
    for (coin_type, (deny_list, owners)) in new_regulated_coin_owners {
        if check_global_pause(&deny_list, object_store, Some(cur_epoch)) {
            return Err(ExecutionError::new(
                ExecutionErrorKind::CoinTypeGlobalPause {
                    coin_type: coin_type.clone(),
                },
                None,
            ));
        }
        for owner in owners {
            if check_address_denied_by_config(&deny_list, owner, object_store, Some(cur_epoch)) {
                return Err(ExecutionError::new(
                    ExecutionErrorKind::AddressDeniedForCoin {
                        address: owner,
                        coin_type,
                    },
                    None,
                ));
            }
        }
    }
    Ok(())
}

pub fn get_per_type_coin_deny_list_v2(
    coin_type: &String,
    object_store: &dyn ObjectStore,
) -> Option<Config> {
    let config_key = DOFWrapper {
        name: ConfigKey {
            per_type_index: DENY_LIST_COIN_TYPE_INDEX,
            per_type_key: coin_type.as_bytes().to_vec(),
        },
    };
    // TODO: Consider caching the config object UID to avoid repeat deserialization.
    let config: Config =
        get_dynamic_field_from_store(object_store, SUI_DENY_LIST_OBJECT_ID, &config_key).ok()?;
    Some(config)
}

pub fn check_address_denied_by_config(
    deny_config: &Config,
    address: SuiAddress,
    object_store: &dyn ObjectStore,
    cur_epoch: Option<EpochId>,
) -> bool {
    let address_key = AddressKey(address);
    read_config_setting(object_store, deny_config, address_key, cur_epoch).unwrap_or(false)
}

pub fn check_global_pause(
    deny_config: &Config,
    object_store: &dyn ObjectStore,
    cur_epoch: Option<EpochId>,
) -> bool {
    let global_pause_key = GlobalPauseKey::new();
    read_config_setting(object_store, deny_config, global_pause_key, cur_epoch).unwrap_or(false)
}

/// Fetches the setting from a particular config.
/// Reads the value of the setting, giving `newer_value` if the current epoch is greater than
/// `newer_value_epoch`, and `older_value_opt` otherwise.
/// If `cur_epoch` is `None`, the `newer_value` is always returned.
fn read_config_setting<K, V>(
    object_store: &dyn ObjectStore,
    config: &Config,
    setting_name: K,
    cur_epoch: Option<EpochId>,
) -> Option<V>
where
    K: MoveTypeTagTrait + Serialize + DeserializeOwned + fmt::Debug,
    V: Clone + Serialize + DeserializeOwned + fmt::Debug,
{
    let setting: Setting<V> = {
        match get_dynamic_field_from_store(object_store, *config.id.object_id(), &setting_name) {
            Ok(setting) => setting,
            Err(_) => return None,
        }
    };
    setting.read_value(cur_epoch).cloned()
}
