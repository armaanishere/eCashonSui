// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use clap::Parser;
use eyre::eyre;
use rocksdb::MultiThreaded;
use std::collections::BTreeMap;
use std::path::PathBuf;
use strum_macros::EnumString;
use sui_core::{
    authority::authority_store_tables::AuthorityStoreTablesReadOnly,
    checkpoints::CheckpointStoreTablesReadOnly,
};
use sui_storage::{
    default_db_options, indexes::IndexStoreReadOnly, lock_service::LockServiceImplReadOnly,
    node_sync_store::NodeSyncStoreReadOnly,
};
use sui_types::crypto::{AuthoritySignInfo, EmptySignInfo};

#[derive(EnumString, Parser, Debug)]
pub enum StoreName {
    Validator,
    Gateway,
    Index,
    LocksService,
    NodeSync,
    Checkpoints,
    Wal,
}
impl std::fmt::Display for StoreName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub fn list_tables(path: PathBuf) -> anyhow::Result<Vec<String>> {
    rocksdb::DBWithThreadMode::<MultiThreaded>::list_cf(&default_db_options(None, None).0, &path)
        .map_err(|e| e.into())
        .map(|q| {
            q.iter()
                .filter_map(|s| {
                    // The `default` table is not used
                    if s != "default" {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect()
        })
}

// TODO: condense this using macro or trait dyn skills
pub fn dump_table(
    store_name: StoreName,
    db_path: PathBuf,
    table_name: &str,
    page_size: u16,
    page_number: usize,
) -> anyhow::Result<BTreeMap<String, String>> {
    match store_name {
        StoreName::Validator => {
            AuthorityStoreTablesReadOnly::<AuthoritySignInfo>::open_tables_read_only(
                db_path, None, None,
            )
            .dump(table_name, page_size, page_number)
        }
        StoreName::Gateway => AuthorityStoreTablesReadOnly::<EmptySignInfo>::open_tables_read_only(
            db_path, None, None,
        )
        .dump(table_name, page_size, page_number),
        StoreName::Index => IndexStoreReadOnly::open_tables_read_only(db_path, None, None).dump(
            table_name,
            page_size,
            page_number,
        ),
        StoreName::LocksService => LockServiceImplReadOnly::open_tables_read_only(
            db_path, None, None,
        )
        .dump(table_name, page_size, page_number),
        StoreName::NodeSync => NodeSyncStoreReadOnly::open_tables_read_only(db_path, None, None)
            .dump(table_name, page_size, page_number),
        StoreName::Checkpoints => CheckpointStoreTablesReadOnly::open_tables_read_only(
            db_path, None, None,
        )
        .dump(table_name, page_size, page_number),
        StoreName::Wal => Err(eyre!(
            "Dumping WAL not yet supported. It requires kmowing the value type"
        )),
    }
    .map_err(|err| anyhow!(err.to_string()))
}

#[cfg(test)]
mod test {
    use sui_core::authority::authority_store_tables::AuthorityStoreTables;
    use sui_types::crypto::AuthoritySignInfo;
    use typed_store::traits::DBMapTableUtil;

    use crate::db_tool::db_dump::{dump_table, list_tables, StoreName};

    #[tokio::test]
    async fn db_dump_population() -> Result<(), anyhow::Error> {
        let primary_path = tempfile::tempdir()?.into_path();

        // Open the DB for writing
        let _: AuthorityStoreTables<AuthoritySignInfo> =
            AuthorityStoreTables::open_tables_read_write(primary_path.clone(), None, None);

        // Get all the tables
        let tables = list_tables(primary_path.clone()).unwrap();

        let mut missing_tables = vec![];
        for t in tables {
            println!("{}", t);
            if dump_table(StoreName::Validator, primary_path.clone(), &t, 0, 0).is_err() {
                missing_tables.push(t);
            }
        }
        if missing_tables.is_empty() {
            return Ok(());
        }
        panic!(
            "{}",
            format!(
                "Missing {} table(s) from DB dump registration function: {:?} \n Update the dump function.",
                missing_tables.len(),
                missing_tables
            )
        );
    }
}
