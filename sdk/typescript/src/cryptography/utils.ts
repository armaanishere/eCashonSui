// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
/* eslint-disable import/no-cycle */

import { fromB64 } from '@mysten/bcs';

import { Ed25519Keypair } from '../keypairs/ed25519/keypair.js';
import { Ed25519PublicKey } from '../keypairs/ed25519/publickey.js';
import { Secp256k1Keypair } from '../keypairs/secp256k1/keypair.js';
import { Secp256k1PublicKey } from '../keypairs/secp256k1/publickey.js';
import { Secp256r1Keypair } from '../keypairs/secp256r1/keypair.js';
import { Secp256r1PublicKey } from '../keypairs/secp256r1/publickey.js';
import { toZkLoginPublicIdentifier } from '../keypairs/zklogin/publickey.js';
import type { ExportedKeypair, Keypair } from './keypair.js';
import { LEGACY_PRIVATE_KEY_SIZE, PRIVATE_KEY_SIZE } from './keypair.js';
import { decodeMultisigStruct, SIGNATURE_SCHEME_TO_PUBLIC_KEY } from './multisig.js';
import type { PublicKey } from './publickey.js';
import type { SignatureScheme } from './signature-scheme.js';
import { parseSerializedSignature } from './signature.js';
import type { SerializedSignature } from './signature.js';

/**
 * Pair of signature and corresponding public key
 */
export type SignaturePubkeyPair = {
	signatureScheme: SignatureScheme;
	/** Base64-encoded signature */
	signature: Uint8Array;
	/** Base64-encoded public key */
	pubKey: PublicKey;
	weight?: number;
};

/// Expects to parse a serialized signature by its signature scheme to a list of signature
/// and public key pairs. The list is of length 1 if it is not multisig.
export function toParsedSignaturePubkeyPair(
	serializedSignature: SerializedSignature,
): SignaturePubkeyPair[] {
	const { signatureScheme, publicKey, signature, multisig, zkLogin } =
		parseSerializedSignature(serializedSignature);

	switch (signatureScheme) {
		case 'MultiSig':
			try {
				return decodeMultisigStruct(multisig);
			} catch (e) {
				// Legacy format multisig do not render.
				throw new Error('legacy multisig viewing unsupported');
			}
		case 'ZkLogin':
			const publicIdentifier = toZkLoginPublicIdentifier(BigInt(zkLogin.addressSeed), zkLogin.iss);
			return [
				{
					signatureScheme,
					signature,
					pubKey: publicIdentifier,
				},
			];
		case 'ED25519':
		case 'Secp256k1':
		case 'Secp256r1':
			const PublicKey = SIGNATURE_SCHEME_TO_PUBLIC_KEY[signatureScheme];
			const pubKey = new PublicKey(publicKey);
			return [
				{
					signatureScheme,
					signature,
					pubKey,
				},
			];
		default:
			throw new Error('Unsupported signature scheme');
	}
}

/// Expects to parse a single signature pubkey pair from the serialized
/// signature. Use this only if multisig is not expected.
export function toSingleSignaturePubkeyPair(
	serializedSignature: SerializedSignature,
): SignaturePubkeyPair {
	const res = toParsedSignaturePubkeyPair(serializedSignature);
	if (res.length !== 1) {
		throw Error('Expected a single signature');
	}
	return res[0];
}

/// Creates specific PublicKey objects from serialized strings, based on their corresponding signature schemes
export function publicKeyFromSerialized(schema: SignatureScheme, pubKey: string): PublicKey {
	if (schema === 'ED25519') {
		return new Ed25519PublicKey(pubKey);
	}
	if (schema === 'Secp256k1') {
		return new Secp256k1PublicKey(pubKey);
	}
	if (schema === 'Secp256r1') {
		return new Secp256r1PublicKey(pubKey);
	}
	throw new Error('Unknown public key schema');
}

/// Reconstructs a `Keypair` object from an exported keypair representation
export function fromExportedKeypair(keypair: ExportedKeypair): Keypair {
	const secretKey = fromB64(keypair.privateKey);
	switch (keypair.schema) {
		case 'ED25519':
			let pureSecretKey = secretKey;
			if (secretKey.length === LEGACY_PRIVATE_KEY_SIZE) {
				// This is a legacy secret key, we need to strip the public key bytes and only read the first 32 bytes
				pureSecretKey = secretKey.slice(0, PRIVATE_KEY_SIZE);
			}
			return Ed25519Keypair.fromSecretKey(pureSecretKey);
		case 'Secp256k1':
			return Secp256k1Keypair.fromSecretKey(secretKey);
		case 'Secp256r1':
			return Secp256r1Keypair.fromSecretKey(secretKey);
		default:
			throw new Error(`Invalid keypair schema ${keypair.schema}`);
	}
}
