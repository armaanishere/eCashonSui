// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::cache::type_cache::TypeCache;

use move_binary_format::{
    errors::{PartialVMError, PartialVMResult},
    file_format::LocalIndex,
};
use move_core_types::{runtime_value::MoveTypeLayout, vm_status::StatusCode};
use move_vm_config::runtime::VMConfig;
use move_vm_types::{
    loaded_data::runtime_types::Type,
    values::{Locals, Reference, VMValueCast, Value},
};

use parking_lot::RwLock;
use tracing::warn;

use std::borrow::Borrow;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

/// Serialized return values from function/script execution
/// Simple struct is designed just to convey meaning behind serialized values
#[derive(Debug)]
pub struct SerializedReturnValues {
    /// The value of any arguments that were mutably borrowed.
    /// Non-mut borrowed values are not included
    pub mutable_reference_outputs: Vec<(LocalIndex, Vec<u8>, MoveTypeLayout)>,
    /// The return values from the function
    pub return_values: Vec<(Vec<u8>, MoveTypeLayout)>,
}

// -------------------------------------------------------------------------------------------------
// Value Serialization and Deserialization
// -------------------------------------------------------------------------------------------------

pub fn deserialize_value(
    type_cache: &RwLock<TypeCache>,
    ty: &Type,
    arg: impl Borrow<[u8]>,
) -> PartialVMResult<Value> {
    let layout = match type_cache.write().type_to_type_layout(ty) {
        Ok(layout) => layout,
        Err(_err) => {
            warn!("[VM] failed to get layout from type");
            return Err(PartialVMError::new(
                StatusCode::INVALID_PARAM_TYPE_FOR_DESERIALIZATION,
            ));
        }
    };

    match Value::simple_deserialize(arg.borrow(), &layout) {
        Some(val) => Ok(val),
        None => {
            warn!("[VM] failed to deserialize argument");
            Err(PartialVMError::new(
                StatusCode::FAILED_TO_DESERIALIZE_ARGUMENT,
            ))
        }
    }
}

pub fn deserialize_args(
    type_cache: &RwLock<TypeCache>,
    vm_config: &VMConfig,
    arg_tys: Vec<Type>,
    serialized_args: Vec<impl Borrow<[u8]>>,
) -> PartialVMResult<(Locals, Vec<Value>)> {
    if arg_tys.len() != serialized_args.len() {
        return Err(
            PartialVMError::new(StatusCode::NUMBER_OF_ARGUMENTS_MISMATCH).with_message(format!(
                "argument length mismatch: expected {} got {}",
                arg_tys.len(),
                serialized_args.len()
            )),
        );
    }

    // Create a list of dummy locals. Each value stored will be used be borrowed and passed
    // by reference to the invoked function
    let mut dummy_locals = Locals::new_from(vec![], arg_tys.len());
    // Arguments for the invoked function. These can be owned values or references
    let deserialized_args = arg_tys
        .into_iter()
        .zip(serialized_args)
        .enumerate()
        .map(|(idx, (arg_ty, arg_bytes))| match &arg_ty {
            Type::MutableReference(inner_t) | Type::Reference(inner_t) => {
                dummy_locals.store_loc(
                    idx,
                    deserialize_value(type_cache, inner_t, arg_bytes)?,
                    vm_config.enable_invariant_violation_check_in_swap_loc,
                )?;
                dummy_locals.borrow_loc(idx)
            }
            _ => deserialize_value(type_cache, &arg_ty, arg_bytes),
        })
        .collect::<PartialVMResult<Vec<_>>>()?;
    Ok((dummy_locals, deserialized_args))
}

pub fn serialize_return_value(
    type_cache: &RwLock<TypeCache>,
    vm_config: &VMConfig,
    ty: &Type,
    value: Value,
) -> PartialVMResult<(Vec<u8>, MoveTypeLayout)> {
    let (ty, value) = match ty {
        Type::Reference(inner) | Type::MutableReference(inner) => {
            let ref_value: Reference = value.cast().map_err(|_err| {
                PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR).with_message(
                    "non reference value given for a reference typed return value".to_string(),
                )
            })?;
            let inner_value = ref_value.read_ref()?;
            (&**inner, inner_value)
        }
        _ => (ty, value),
    };

    let layout = if vm_config.rethrow_serialization_type_layout_errors {
        type_cache.write().type_to_type_layout(ty)?
    } else {
        type_cache.write().type_to_type_layout(ty).map_err(|_err| {
            PartialVMError::new(StatusCode::VERIFICATION_ERROR).with_message(
                "entry point functions cannot have non-serializable return types".to_string(),
            )
        })?
    };

    let bytes = value.simple_serialize(&layout).ok_or_else(|| {
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message("failed to serialize return values".to_string())
    })?;
    Ok((bytes, layout))
}

pub fn serialize_return_values(
    type_cache: &RwLock<TypeCache>,
    vm_config: &VMConfig,
    return_types: &[Type],
    return_values: Vec<Value>,
) -> PartialVMResult<Vec<(Vec<u8>, MoveTypeLayout)>> {
    if return_types.len() != return_values.len() {
        return Err(
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                format!(
                    "declared {} return types, but got {} return values",
                    return_types.len(),
                    return_values.len()
                ),
            ),
        );
    }

    return_types
        .iter()
        .zip(return_values)
        .map(|(ty, value)| serialize_return_value(type_cache, vm_config, ty, value))
        .collect()
}
