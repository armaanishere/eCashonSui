// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("protos/narwhal.proto")?;
    Ok(())
}
