// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module sui::crypto_tests {
    use sui::crypto;
    use sui::elliptic_curve as ec;
    #[test]
    fun test_ecrecover_pubkey() {
        // test case generated against https://docs.rs/secp256k1/latest/secp256k1/
        let hashed_msg = vector[87, 202, 161, 118, 175, 26, 192, 67, 60, 93, 243, 14, 141, 171, 205, 46, 193, 175, 30, 146, 162, 110, 206, 213, 247, 25, 184, 132, 88, 119, 124, 214];
        let sig = vector[132, 220, 128, 67, 151, 154, 45, 143, 50, 56, 176, 134, 137, 58, 223, 166, 191, 230, 178, 184, 123, 11, 19, 69, 59, 205, 72, 206, 153, 187, 184, 7, 16, 74, 73, 45, 38, 238, 81, 96, 138, 225, 235, 143, 95, 142, 185, 56, 99, 3, 97, 27, 66, 99, 79, 225, 139, 21, 67, 254, 78, 251, 176, 176, 0];
        let pubkey_bytes = vector[2, 2, 87, 224, 47, 124, 255, 117, 223, 91, 188, 190, 151, 23, 241, 173, 148, 107, 20, 103, 63, 155, 108, 151, 251, 152, 205, 205, 239, 71, 224, 86, 9];

        let pubkey = crypto::ecrecover(sig, hashed_msg);
        assert!(pubkey == pubkey_bytes, 0);
    }

    #[test]
    #[expected_failure(abort_code = 0)]
    fun test_ecrecover_pubkey_fail_to_recover() {
        let hashed_msg = vector[0];
        let sig = vector[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        crypto::ecrecover(sig, hashed_msg);
    }

    #[test]
    #[expected_failure(abort_code = 1)]
    fun test_ecrecover_pubkey_invalid_sig() {
        let hashed_msg = vector[0];
        // incorrect length sig
        let sig = vector[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        crypto::ecrecover(sig, hashed_msg);
    }

    #[test]
    fun test_keccak256_hash() {
        let msg = b"hello world!";
        let hashed_msg_bytes = vector[87, 202, 161, 118, 175, 26, 192, 67, 60, 93, 243, 14, 141, 171, 205, 46, 193, 175, 30, 146, 162, 110, 206, 213, 247, 25, 184, 132, 88, 119, 124, 214];

        let hashed_msg = crypto::keccak256(msg);
        assert!(hashed_msg == hashed_msg_bytes, 0);
    }
    
    #[test]
    fun test_bls12381_valid_sig() {
        let msg = vector[1, 1, 1, 1, 1];
        let pk = vector[141, 241, 1, 96, 111, 145, 243, 202, 215, 245, 75, 138, 255, 15, 15, 100, 196, 28, 72, 45, 155, 159, 159, 232, 29, 43, 96, 123, 197, 246, 17, 189, 250, 128, 23, 207, 4, 180, 123, 68, 178, 34, 195, 86, 239, 85, 95, 189, 17, 5, 140, 82, 192, 119, 245, 167, 236, 106, 21, 204, 253, 99, 159, 220, 155, 212, 125, 0, 90, 17, 29, 214, 205, 184, 192, 47, 228, 150, 8, 223, 85, 163, 201, 130, 41, 134, 173, 11, 134, 189, 234, 58, 191, 223, 228, 100];
        let sig = vector[144, 142, 52, 95, 46, 40, 3, 205, 148, 26, 232, 140, 33, 140, 150, 25, 66, 51, 201, 5, 63, 161, 188, 165, 33, 36, 120, 125, 60, 202, 20, 28, 54, 66, 157, 118, 82, 67, 90, 130, 12, 114, 153, 45, 94, 238, 99, 23];
        let verify = crypto::bls12381_verify_g1_sig(sig, pk, msg);
        assert!(verify == true, 0)
    }

    #[test]
    fun test_bls12381_invalid_sig() {
        let msg = vector[2, 1, 1, 1, 1];
        let pk = vector[141, 241, 1, 96, 111, 145, 243, 202, 215, 245, 75, 138, 255, 15, 15, 100, 196, 28, 72, 45, 155, 159, 159, 232, 29, 43, 96, 123, 197, 246, 17, 189, 250, 128, 23, 207, 4, 180, 123, 68, 178, 34, 195, 86, 239, 85, 95, 189, 17, 5, 140, 82, 192, 119, 245, 167, 236, 106, 21, 204, 253, 99, 159, 220, 155, 212, 125, 0, 90, 17, 29, 214, 205, 184, 192, 47, 228, 150, 8, 223, 85, 163, 201, 130, 41, 134, 173, 11, 134, 189, 234, 58, 191, 223, 228, 100];
        let sig = vector[144, 142, 52, 95, 46, 40, 3, 205, 148, 26, 232, 140, 33, 140, 150, 25, 66, 51, 201, 5, 63, 161, 188, 165, 33, 36, 120, 125, 60, 202, 20, 28, 54, 66, 157, 118, 82, 67, 90, 130, 12, 114, 153, 45, 94, 238, 99, 23];
        let verify = crypto::bls12381_verify_g1_sig(sig, pk, msg);
        assert!(verify == false, 0)
    }

    #[test]
    fun test_bls12381_invalid_signature_key_length() {
        let msg = vector[2, 1, 1, 1, 1];
        let pk = vector[96, 111, 145, 243, 202, 215, 245, 75, 138, 255, 15, 15, 100, 196, 28, 72, 45, 155, 159, 159, 232, 29, 43, 96, 123, 197, 246, 17, 189, 250, 128, 23, 207, 4, 180, 123, 68, 178, 34, 195, 86, 239, 85, 95, 189, 17, 5, 140, 82, 192, 119, 245, 167, 236, 106, 21, 204, 253, 99, 159, 220, 155, 212, 125, 0, 90, 17, 29, 214, 205, 184, 192, 47, 228, 150, 8, 223, 85, 163, 201, 130, 41, 134, 173, 11, 134, 189, 234, 58, 191, 223, 228, 100];
        let sig = vector[144, 142, 52, 0, 46, 40, 3, 205, 148, 26, 232, 140, 33, 140, 150, 25, 66, 51, 201, 5, 63, 161, 188, 165, 33, 36, 120, 125, 60, 202, 20, 28, 54, 66, 157, 118, 82, 67, 90, 130, 12, 114, 153, 45, 94, 238, 99, 23];
        let verify = crypto::bls12381_verify_g1_sig(sig, pk, msg);
        assert!(verify == false, 0)
    }

    #[test]
    fun test_bls12381_invalid_public_key_length() {
        let msg = vector[2, 1, 1, 1, 1];
        let pk = vector[96, 111, 145, 243, 202, 215, 245, 75, 138, 255, 15, 15, 100, 196, 28, 72, 45, 155, 159, 159, 232, 29, 43, 96, 123, 197, 246, 17, 189, 250, 128, 23, 207, 4, 180, 123, 68, 178, 34, 195, 86, 239, 85, 95, 189, 17, 5, 140, 82, 192, 119, 245, 167, 236, 106, 21, 204, 253, 99, 159, 220, 155, 212, 125, 0, 90, 17, 29, 214, 205, 184, 192, 47, 228, 150, 8, 223, 85, 163, 201, 130, 41, 134, 173, 11, 134, 189, 234, 58, 191, 223, 228, 100];
        let sig = vector[144, 142, 52, 95, 46, 40, 3, 205, 148, 26, 232, 140, 33, 140, 150, 25, 66, 51, 201, 5, 63, 161, 188, 165, 33, 36, 120, 125, 60, 202, 20, 28, 54, 66, 157, 118, 82, 67, 90, 130, 12, 114, 153, 45, 94, 238, 99, 23];
        let verify = crypto::bls12381_verify_g1_sig(sig, pk, msg);
        assert!(verify == false, 0)
    }

    #[test]
    fun test_ristretto_point_addition() {
        let committed_value_1 = 1000u64;
        let blinding_value_1 = 100u64;
        let committed_value_2 = 500u64;
        let blinding_value_2 = 200u64;

        let committed_sum = committed_value_1 + committed_value_2;
        let blinding_sum = blinding_value_1 + blinding_value_2;

        let point_1 = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_value_1),
            ec::new_scalar_from_u64(blinding_value_1)
        );

        let point_2 = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_value_2),
            ec::new_scalar_from_u64(blinding_value_2)
        );

        let point_sum_reference = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_sum),
            ec::new_scalar_from_u64(blinding_sum)
        );

        let point_sum = ec::add(&point_1, &point_2);

        assert!(ec::bytes(&point_sum) == ec::bytes(&point_sum_reference), 0)
    }

    #[test]
    fun test_ristretto_point_subtraction() {
        let committed_value_1 = 1000u64;
        let blinding_value_1 = 100u64;
        let committed_value_2 = 500u64;
        let blinding_value_2 = 50u64;

        let committed_sum = committed_value_1 - committed_value_2;
        let blinding_sum = blinding_value_1 - blinding_value_2;

        let point_1 = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_value_1),
            ec::new_scalar_from_u64(blinding_value_1)
        );

        let point_2 = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_value_2),
            ec::new_scalar_from_u64(blinding_value_2)
        );

        let point_diff_reference = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_sum),
            ec::new_scalar_from_u64(blinding_sum)
        );

        let point_diff = ec::add(&point_1, &point_2);

        assert!(ec::bytes(&point_diff) == ec::bytes(&point_diff_reference), 0)
    }

    #[test]
    fun test_pedersen_commitment() {
        // These are generated elsewhere;
        let commitment = vector[224, 131, 28, 42, 140, 170, 172, 201, 243, 54, 153, 119, 106, 97, 215, 123, 64, 125, 6, 93, 9, 1, 78, 186, 6, 18, 64, 219, 210, 225, 125, 113];

        let committed_value = 1000u64;
        let blinding_factor = 10u64;

        let point = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_value),
            ec::new_scalar_from_u64(blinding_factor)
        );

        assert!(commitment == ec::bytes(&point), 0);
    }

    #[test]
    fun test_bulletproof_standard_0_2pow64_proof() {
        // These are generated elsewhere;
        let bulletproof = vector[
            222, 248, 245, 34, 209, 240, 49, 64, 37, 205, 174, 236, 167, 140, 136, 230, 139, 12, 184, 101, 238, 245, 71, 131, 3, 75, 63, 156, 132, 61, 81, 78, 216, 165, 254, 193, 92, 55, 38, 84, 116, 131, 201, 75, 255, 210, 224, 57, 212, 94, 24, 145, 57, 81, 19, 202, 3, 33, 211, 226, 82, 0, 207, 4, 208, 153, 197, 69, 170, 208, 107, 254, 191, 225, 171, 130, 101, 91, 251, 163, 15, 4, 84, 99, 140, 84, 13, 171, 26, 66, 79, 144, 116, 182, 167, 31, 192, 45, 135, 143, 107, 138, 171, 48, 173, 141, 45, 26, 115, 187, 60, 82, 130, 159, 207, 118, 43, 176, 28, 1, 231, 180, 203, 211, 167, 238, 193, 27, 66, 225, 58, 187, 56, 10, 162, 127, 30, 83, 147, 177, 116, 243, 11, 107, 241, 23, 236, 240, 144, 136, 137, 253, 52, 135, 170, 120, 221, 125, 125, 4, 5, 10, 45, 79, 15, 86, 218, 136, 48, 48, 240, 17, 124, 108, 44, 0, 58, 231, 254, 224, 99, 30, 198, 103, 50, 132, 7, 227, 171, 79, 193, 14, 99, 43, 24, 249, 119, 153, 49, 31, 167, 165, 153, 144, 20, 241, 151, 115, 242, 62, 163, 151, 218, 194, 175, 117, 161, 188, 105, 112, 13, 122, 57, 2, 226, 11, 244, 165, 216, 20, 53, 251, 110, 37, 17, 103, 229, 119, 100, 188, 62, 245, 36, 35, 179, 119, 146, 154, 17, 174, 135, 163, 69, 213, 200, 101, 178, 74, 23, 8, 31, 140, 208, 9, 118, 163, 55, 43, 247, 228, 13, 15, 243, 83, 185, 38, 27, 40, 9, 180, 98, 21, 232, 94, 254, 191, 150, 122, 186, 248, 107, 155, 120, 58, 242, 253, 130, 31, 211, 136, 201, 51, 213, 159, 97, 142, 140, 158, 138, 31, 65, 238, 39, 196, 207, 85, 65, 105, 210, 91, 46, 45, 36, 137, 225, 247, 5, 105, 164, 23, 152, 220, 222, 228, 200, 107, 42, 44, 113, 100, 209, 209, 169, 158, 234, 234, 221, 138, 51, 90, 126, 52, 84, 178, 69, 19, 55, 240, 193, 215, 175, 10, 86, 167, 149, 49, 129, 22, 84, 162, 147, 5, 170, 110, 231, 158, 201, 221, 149, 54, 251, 97, 113, 84, 220, 201, 208, 66, 96, 124, 127, 244, 35, 227, 124, 46, 22, 75, 21, 112, 103, 122, 99, 252, 42, 238, 64, 190, 27, 166, 101, 43, 29, 231, 196, 80, 120, 1, 80, 131, 94, 130, 27, 254, 238, 157, 28, 168, 76, 21, 84, 177, 133, 191, 198, 69, 2, 214, 223, 160, 47, 36, 164, 236, 247, 93, 61, 39, 24, 157, 46, 174, 189, 140, 170, 52, 92, 45, 22, 71, 177, 183, 91, 221, 92, 123, 40, 201, 16, 66, 157, 198, 54, 119, 251, 102, 143, 89, 188, 110, 12, 248, 7, 124, 222, 92, 66, 249, 74, 67, 183, 138, 235, 40, 203, 27, 152, 11, 64, 96, 129, 50, 249, 156, 189, 233, 182, 226, 120, 209, 135, 9, 24, 10, 109, 34, 84, 234, 148, 72, 2, 253, 66, 242, 81, 63, 174, 60, 103, 65, 151, 143, 46, 251, 188, 228, 38, 19, 143, 246, 185, 126, 158, 13, 224, 84, 174, 19, 72, 70, 80, 109, 113, 52, 206, 31, 83, 61, 186, 44, 95, 114, 157, 123, 15, 165, 58, 47, 7, 78, 86, 149, 111, 235, 142, 75, 250, 141, 40, 85, 137, 237, 61, 233, 229, 142, 196, 47, 60, 95, 191, 202, 81, 153, 39, 229, 11, 225, 209, 212, 115, 175, 78, 3, 1, 87, 165, 107, 144, 213, 166, 166, 234, 42, 72, 200, 27, 19, 154, 201, 124, 233, 165, 201, 41, 103, 235, 143, 175, 200, 55, 64, 33, 120, 143, 233, 163, 157, 145, 3, 162, 228, 232, 81, 110, 194, 46, 94, 214, 145, 137, 57, 2, 128, 225, 40, 23, 210, 71, 172, 32, 57, 127, 32, 110, 221, 252, 161, 146, 112, 140, 0
        ];

        let committed_value = 1000u64;
        let blinding_factor = 10u64;

        let point = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_value),
            ec::new_scalar_from_u64(blinding_factor)
        );

        crypto::verify_full_range_proof(bulletproof, point);
    }

    #[test]
    #[expected_failure(abort_code = 0)]
    fun test_bulletproof_standard_0_2pow64_invalid_proof() {
        // These are generated elsewhere;
        let bulletproof = vector[
            0, 0, 0, 0, 0, 240, 49, 64, 37, 205, 174, 236, 167, 140, 136, 230, 139, 12, 184, 101, 238, 245, 71, 131, 3, 75, 63, 156, 132, 61, 81, 78, 216, 165, 254, 193, 92, 55, 38, 84, 116, 131, 201, 75, 255, 210, 224, 57, 212, 94, 24, 145, 57, 81, 19, 202, 3, 33, 211, 226, 82, 0, 207, 4, 208, 153, 197, 69, 170, 208, 107, 254, 191, 225, 171, 130, 101, 91, 251, 163, 15, 4, 84, 99, 140, 84, 13, 171, 26, 66, 79, 144, 116, 182, 167, 31, 192, 45, 135, 143, 107, 138, 171, 48, 173, 141, 45, 26, 115, 187, 60, 82, 130, 159, 207, 118, 43, 176, 28, 1, 231, 180, 203, 211, 167, 238, 193, 27, 66, 225, 58, 187, 56, 10, 162, 127, 30, 83, 147, 177, 116, 243, 11, 107, 241, 23, 236, 240, 144, 136, 137, 253, 52, 135, 170, 120, 221, 125, 125, 4, 5, 10, 45, 79, 15, 86, 218, 136, 48, 48, 240, 17, 124, 108, 44, 0, 58, 231, 254, 224, 99, 30, 198, 103, 50, 132, 7, 227, 171, 79, 193, 14, 99, 43, 24, 249, 119, 153, 49, 31, 167, 165, 153, 144, 20, 241, 151, 115, 242, 62, 163, 151, 218, 194, 175, 117, 161, 188, 105, 112, 13, 122, 57, 2, 226, 11, 244, 165, 216, 20, 53, 251, 110, 37, 17, 103, 229, 119, 100, 188, 62, 245, 36, 35, 179, 119, 146, 154, 17, 174, 135, 163, 69, 213, 200, 101, 178, 74, 23, 8, 31, 140, 208, 9, 118, 163, 55, 43, 247, 228, 13, 15, 243, 83, 185, 38, 27, 40, 9, 180, 98, 21, 232, 94, 254, 191, 150, 122, 186, 248, 107, 155, 120, 58, 242, 253, 130, 31, 211, 136, 201, 51, 213, 159, 97, 142, 140, 158, 138, 31, 65, 238, 39, 196, 207, 85, 65, 105, 210, 91, 46, 45, 36, 137, 225, 247, 5, 105, 164, 23, 152, 220, 222, 228, 200, 107, 42, 44, 113, 100, 209, 209, 169, 158, 234, 234, 221, 138, 51, 90, 126, 52, 84, 178, 69, 19, 55, 240, 193, 215, 175, 10, 86, 167, 149, 49, 129, 22, 84, 162, 147, 5, 170, 110, 231, 158, 201, 221, 149, 54, 251, 97, 113, 84, 220, 201, 208, 66, 96, 124, 127, 244, 35, 227, 124, 46, 22, 75, 21, 112, 103, 122, 99, 252, 42, 238, 64, 190, 27, 166, 101, 43, 29, 231, 196, 80, 120, 1, 80, 131, 94, 130, 27, 254, 238, 157, 28, 168, 76, 21, 84, 177, 133, 191, 198, 69, 2, 214, 223, 160, 47, 36, 164, 236, 247, 93, 61, 39, 24, 157, 46, 174, 189, 140, 170, 52, 92, 45, 22, 71, 177, 183, 91, 221, 92, 123, 40, 201, 16, 66, 157, 198, 54, 119, 251, 102, 143, 89, 188, 110, 12, 248, 7, 124, 222, 92, 66, 249, 74, 67, 183, 138, 235, 40, 203, 27, 152, 11, 64, 96, 129, 50, 249, 156, 189, 233, 182, 226, 120, 209, 135, 9, 24, 10, 109, 34, 84, 234, 148, 72, 2, 253, 66, 242, 81, 63, 174, 60, 103, 65, 151, 143, 46, 251, 188, 228, 38, 19, 143, 246, 185, 126, 158, 13, 224, 84, 174, 19, 72, 70, 80, 109, 113, 52, 206, 31, 83, 61, 186, 44, 95, 114, 157, 123, 15, 165, 58, 47, 7, 78, 86, 149, 111, 235, 142, 75, 250, 141, 40, 85, 137, 237, 61, 233, 229, 142, 196, 47, 60, 95, 191, 202, 81, 153, 39, 229, 11, 225, 209, 212, 115, 175, 78, 3, 1, 87, 165, 107, 144, 213, 166, 166, 234, 42, 72, 200, 27, 19, 154, 201, 124, 233, 165, 201, 41, 103, 235, 143, 175, 200, 55, 64, 33, 120, 143, 233, 163, 157, 145, 3, 162, 228, 232, 81, 110, 194, 46, 94, 214, 145, 137, 57, 2, 128, 225, 40, 23, 210, 71, 172, 32, 57, 127, 32, 110, 221, 252, 161, 146, 112, 140, 0
        ];

        let committed_value = 1000u64;
        let blinding_factor = 10u64;

        let point = ec::create_pedersen_commitment(
            ec::new_scalar_from_u64(committed_value),
            ec::new_scalar_from_u64(blinding_factor)
        );

        crypto::verify_full_range_proof(bulletproof, point);
    }
}
