// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

fn main() {
    println!("sui-test-validator binary has been deprecated in favor of sui start, which is a more powerful command that allows you to start the local network with more options.

How to install/build the sui binary IF:
    A: you only need the basic functionality, so just faucet and no persistence (no indexer, no GraphQL service), build from source as usual (cargo build --bin sui) or download latest archive from release archives (starting from testnet v1.28.1 or devnet v1.29) and use sui binary.
    B: you need to also start an indexer (--with-indexer ), or a graphql service (--with-graphql), you either:
    - download latest archive from release archives (starting from testnet v1.28.1 or devnet v1.29) and use sui-pg binary.
  OR
    - build from source. This requires passing the indexer feature when building the sui binary, as well as having libpq/postgresql dependencies installed (just as when using sui-test-validator):
        - cargo build --bin sui --features indexer
        - cargo run --bin sui -- start --with-faucet --force-regenesis --with-indexer --with-graphql

Running the local network:
 - In the simplest form, you can replace sui-test-validator with sui start --with-faucet --force-regenesis. This will create a network from a new genesis and start a faucet (127.0.0.1:9123). This will not persist state.
 - Use sui start --help to get all the flags and options.
 - Use the drop-in replacement script: sui/scripts/sui-test-validator.sh and pass in all the flags/options as you used to.

If you used to use sui start before, the behavior is preserved so it should keep working as it used to.

We recommend to migrate to using sui start instead of using the script. To do so, use sui start \
--help to see all the flags and options. You can also use the following options to start the local \
network with more features:
  * --with-indexer --> to start the indexer on the default host and port. Note that this requires \
a Postgres database to be running locally, or you need to set the different options to connect to a \
remote indexer database.
  * --with-graphql --> to start the GraphQL server on the default host and port");
}
