// Copyright (c) Facebook, Inc. and its affiliates.
// SPDX-License-Identifier: Apache-2.0

use crate::transport::NetworkProtocol;
use fastpay_core::client::ClientState;
use fastx_types::{
    base_types::*,
    messages::{Address, CertifiedOrder, OrderKind},
};

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorityConfig {
    pub network_protocol: NetworkProtocol,
    #[serde(
        serialize_with = "address_as_base64",
        deserialize_with = "address_from_base64"
    )]
    pub address: FastPayAddress,
    pub host: String,
    pub base_port: u32,
    pub num_shards: u32,
    pub database_path : String,
}

impl AuthorityConfig {
    pub fn print(&self) {
        let data = serde_json::to_string(self).unwrap();
        println!("{}", data);
    }
}

#[derive(Serialize, Deserialize)]
pub struct AuthorityServerConfig {
    pub authority: AuthorityConfig,
    pub key: KeyPair,
}

impl AuthorityServerConfig {
    pub fn read(path: &str) -> Result<Self, std::io::Error> {
        let data = fs::read(path)?;
        Ok(serde_json::from_slice(data.as_slice())?)
    }

    pub fn write(&self, path: &str) -> Result<(), std::io::Error> {
        let file = OpenOptions::new().create(true).write(true).open(path)?;
        let mut writer = BufWriter::new(file);
        let data = serde_json::to_string_pretty(self).unwrap();
        writer.write_all(data.as_ref())?;
        writer.write_all(b"\n")?;
        Ok(())
    }
}

pub struct CommitteeConfig {
    pub authorities: Vec<AuthorityConfig>,
}

impl CommitteeConfig {
    pub fn read(path: &str) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let stream = serde_json::Deserializer::from_reader(reader).into_iter();
        Ok(Self {
            authorities: stream.filter_map(Result::ok).collect(),
        })
    }

    pub fn write(&self, path: &str) -> Result<(), std::io::Error> {
        let file = OpenOptions::new().create(true).write(true).open(path)?;
        let mut writer = BufWriter::new(file);
        for config in &self.authorities {
            serde_json::to_writer(&mut writer, config)?;
            writer.write_all(b"\n")?;
        }
        Ok(())
    }

    pub fn voting_rights(&self) -> BTreeMap<AuthorityName, usize> {
        let mut map = BTreeMap::new();
        for authority in &self.authorities {
            map.insert(authority.address, 1);
        }
        map
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserAccount {
    #[serde(
        serialize_with = "address_as_base64",
        deserialize_with = "address_from_base64"
    )]
    pub address: FastPayAddress,
    pub key: KeyPair,
    pub object_ids: BTreeMap<ObjectID, SequenceNumber>,
    pub sent_certificates: Vec<CertifiedOrder>,
    pub received_certificates: Vec<CertifiedOrder>,
}

impl UserAccount {
    pub fn new(object_ids: Vec<ObjectID>) -> Self {
        let (address, key) = get_key_pair();
        Self {
            address,
            key,
            object_ids: object_ids
                .into_iter()
                .map(|object_id| (object_id, SequenceNumber::new()))
                .collect(),
            sent_certificates: Vec::new(),
            received_certificates: Vec::new(),
        }
    }
}

pub struct AccountsConfig {
    accounts: BTreeMap<FastPayAddress, UserAccount>,
}

impl AccountsConfig {
    pub fn get(&self, address: &FastPayAddress) -> Option<&UserAccount> {
        self.accounts.get(address)
    }

    pub fn insert(&mut self, account: UserAccount) {
        self.accounts.insert(account.address, account);
    }

    pub fn num_accounts(&self) -> usize {
        self.accounts.len()
    }

    pub fn accounts_mut(&mut self) -> impl Iterator<Item = &mut UserAccount> {
        self.accounts.values_mut()
    }

    pub fn update_from_state<A>(&mut self, state: &ClientState<A>) {
        let account = self
            .accounts
            .get_mut(&state.address())
            .expect("Updated account should already exist");
        account.object_ids = state.object_ids().clone();
        account.sent_certificates = state.sent_certificates().clone();
        account.received_certificates = state.received_certificates().cloned().collect();
    }

    pub fn update_for_received_transfer(&mut self, certificate: CertifiedOrder) {
        match &certificate.order.kind {
            OrderKind::Transfer(transfer) => {
                if let Address::FastPay(recipient) = &transfer.recipient {
                    if let Some(config) = self.accounts.get_mut(recipient) {
                        if let Err(position) = config
                            .received_certificates
                            .binary_search_by_key(&certificate.key(), CertifiedOrder::key)
                        {
                            config.received_certificates.insert(position, certificate)
                        }
                    }
                }
            }
            OrderKind::Publish(_) | OrderKind::Call(_) => {
                unimplemented!("update_for_received_transfer of Call or Publish")
            }
        }
    }

    pub fn read_or_create(path: &str) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(path)?;
        let reader = BufReader::new(file);
        let stream = serde_json::Deserializer::from_reader(reader).into_iter();
        Ok(Self {
            accounts: stream
                .filter_map(Result::ok)
                .map(|account: UserAccount| (account.address, account))
                .collect(),
        })
    }

    pub fn write(&self, path: &str) -> Result<(), std::io::Error> {
        let file = OpenOptions::new().write(true).open(path)?;
        let mut writer = BufWriter::new(file);
        for account in self.accounts.values() {
            serde_json::to_writer(&mut writer, account)?;
            writer.write_all(b"\n")?;
        }
        Ok(())
    }
}

pub struct InitialStateConfig {
    pub accounts: Vec<(FastPayAddress, ObjectID)>,
}

impl InitialStateConfig {
    pub fn read(path: &str) -> Result<Self, anyhow::Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut accounts = Vec::new();
        for line in reader.lines() {
            let line = line?;
            let elements = line.split(':').collect::<Vec<_>>();
            if elements.len() != 2 {
                anyhow::bail!("expecting two columns separated with ':'")
            }
            let address = decode_address(elements[0])?;
            let object_id = ObjectID::from_hex_literal(elements[1])?;
            accounts.push((address, object_id));
        }
        Ok(Self { accounts })
    }

    pub fn write(&self, path: &str) -> Result<(), std::io::Error> {
        let file = OpenOptions::new().create(true).write(true).open(path)?;
        let mut writer = BufWriter::new(file);
        for (address, object_id) in &self.accounts {
            writeln!(writer, "{}:{}", encode_address(address), object_id,)?;
        }
        Ok(())
    }
}
