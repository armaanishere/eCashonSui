// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import crypto from 'node:crypto';
import { toHex } from '@mysten/bcs';
import type { PublicKey } from '@mysten/sui/cryptography';
import { toPaddedBigEndianBytes } from '@mysten/sui/zklogin';
import { base64url } from 'jose';

import { poseidonHash } from './poseidon.js';

export const NONCE_LENGTH = 27;

function toBigIntBE(bytes: Uint8Array) {
	const hex = toHex(bytes);
	if (hex.length === 0) {
		return BigInt(0);
	}
	return BigInt(`0x${hex}`);
}

export function generateRandomness() {
	return String(toBigIntBE(crypto.getRandomValues(new Uint8Array(16))));
}

export function generateNonce(publicKey: PublicKey, maxEpoch: number, randomness: bigint | string) {
	const publicKeyBytes = toBigIntBE(publicKey.toSuiBytes());
	const eph_public_key_0 = publicKeyBytes / 2n ** 128n;
	const eph_public_key_1 = publicKeyBytes % 2n ** 128n;
	const bigNum = poseidonHash([eph_public_key_0, eph_public_key_1, maxEpoch, BigInt(randomness)]);
	const Z = toPaddedBigEndianBytes(bigNum, 20);
	const nonce = base64url.encode(Z);
	if (nonce.length !== NONCE_LENGTH) {
		throw new Error(`Length of nonce ${nonce} (${nonce.length}) is not equal to ${NONCE_LENGTH}`);
	}
	return nonce;
}
