// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

module sui::bls12381 {
    friend sui::validator;

    /// @param signature: A 48-bytes signature that is a point on the G1 subgroup.
    /// @param public_key: A 96-bytes public key that is a point on the G2 subgroup.
    /// @param msg: The message that we test the signature against.
    ///
    /// If the signature is a valid BLS12381 signature of the message and public key, return true.
    /// Otherwise, return false.
    public native fun bls12381_min_sig_verify(signature: &vector<u8>, public_key: &vector<u8>, msg: &vector<u8>): bool;

    /// @param signature: A 48-bytes signature that is a point on the G1 subgroup.
    /// @param public_key: A 96-bytes public key that is a point on the G2 subgroup.
    /// @param msg: The message that we test the signature against.
    /// @param domain: The domain that the signature is tested again. We essentially prepend this to the message.
    ///
    /// If the signature is a valid Ed25519 signature of the message and public key, return true.
    /// Otherwise, return false.
    public(friend) fun bls12381_min_sig_verify_with_domain(
        signature: &vector<u8>,
        public_key: &vector<u8>,
        msg: vector<u8>,
        domain: vector<u8>
    ): bool {
        std::vector::append(&mut domain, msg);
        bls12381_min_sig_verify(signature, public_key, &domain)
    }
}
