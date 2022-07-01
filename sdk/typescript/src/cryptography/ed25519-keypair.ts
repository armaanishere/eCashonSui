// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import nacl from 'tweetnacl';
import bip39 from 'bip39-light';
import createHmac from 'create-hmac';
import { Base64DataBuffer } from '../serialization/base64';
import { Keypair } from './keypair';
import { PublicKey } from './publickey';
import { TextEncoder } from 'util';

/**
 * Ed25519 Keypair data
 */
export interface Ed25519KeypairData {
  publicKey: Uint8Array;
  secretKey: Uint8Array;
}

/**
 * Derive Path data
 */
interface Keys {
  key: Buffer;
  chainCode: Buffer;
};

/**
 * An Ed25519 Keypair used for signing transactions.
 */
export class Ed25519Keypair implements Keypair {
  private keypair: Ed25519KeypairData;

  /**
   * Create a new keypair instance.
   * Generate random keypair if no {@link Ed25519Keypair} is provided.
   *
   * @param keypair ed25519 keypair
   */
  constructor(keypair?: Ed25519KeypairData) {
    if (keypair) {
      this.keypair = keypair;
    } else {
      this.keypair = nacl.sign.keyPair();
    }
  }

  /**
   * Generate a new random keypair
   */
  static generate(): Ed25519Keypair {
    return new Ed25519Keypair(nacl.sign.keyPair());
  }

  /**
   * Create a keypair from a raw secret key byte array.
   *
   * This method should only be used to recreate a keypair from a previously
   * generated secret key. Generating keypairs from a random seed should be done
   * with the {@link Keypair.fromSeed} method.
   *
   * @throws error if the provided secret key is invalid and validation is not skipped.
   *
   * @param secretKey secret key byte array
   * @param options: skip secret key validation
   */
  static fromSecretKey(
    secretKey: Uint8Array,
    options?: { skipValidation?: boolean }
  ): Ed25519Keypair {
    const keypair = nacl.sign.keyPair.fromSecretKey(secretKey);
    if (!options || !options.skipValidation) {
      const encoder = new TextEncoder();
      const signData = encoder.encode('sui validation');
      const signature = nacl.sign.detached(signData, keypair.secretKey);
      if (!nacl.sign.detached.verify(signData, signature, keypair.publicKey)) {
        throw new Error('provided secretKey is invalid');
      }
    }
    return new Ed25519Keypair(keypair);
  }

  /**
   * Generate a keypair from a 32 byte seed.
   *
   * @param seed seed byte array
   */
  static fromSeed(seed: Uint8Array): Ed25519Keypair {
    return new Ed25519Keypair(nacl.sign.keyPair.fromSeed(seed));
  }

  /**
   * Test Derive Path from path string
   *
   * @param path path string (`m/44'/784'/0'/0'/0'`)
   */
  static isValidPath = (path: string): boolean => {
    if (!new RegExp("^m\\/44+'\\/784+'\\/[0-9]+'\\/[0-9]+'\\/[0-9]+'+$").test(path)) {
        return false;
    }
    return !path
        .split('/')
        .slice(1)
        .map((val: string): string => val.replace("'", ''))
        .some(isNaN as any);
  };

  private static CKDPriv = ({ key, chainCode }: Keys, index: number): Keys => {
    const indexBuffer = Buffer.allocUnsafe(4);
    indexBuffer.writeUInt32BE(index, 0);

    const data = Buffer.concat([Buffer.alloc(1, 0), key, indexBuffer]);

    const I = createHmac('sha512', chainCode)
        .update(data)
        .digest();
    const IL = I.slice(0, 32);
    const IR = I.slice(32);
    return {
        key: IL,
        chainCode: IR,
    };
  };

  /**
   * Generate a keypair from mnemonics.
   *
   * @param path path string (`m/44'/784'/0'/0'/0'`)
   * @param mnemonics: mnemonics is a word seed phrase
   */
  static fromDerivePath(path: string, mnemonics: string): Ed25519Keypair {
    if (!Ed25519Keypair.isValidPath(path)) {
      throw new Error('Invalid derivation path');
    }

    const normalizeMnemonics = mnemonics
        .trim()
        .split(/\s+/)
        .map((part) => part.toLowerCase())
        .join(' ');
    const seed = bip39.mnemonicToSeed(normalizeMnemonics);

    const hmac = createHmac('sha512', 'ed25519 seed');
    const I = hmac.update(seed).digest();
    const key = I.slice(0, 32);
    const chainCode = I.slice(32);

    const segments = path
        .split('/')
        .slice(1)
        .map((val: string): string => val.replace("'", ''))
        .map(el => parseInt(el, 10));
    
    const node = segments.reduce(
          (parentKeys, segment) => Ed25519Keypair.CKDPriv(parentKeys, segment + 0x80000000),
          { key, chainCode },
      )

    return Ed25519Keypair.fromSeed(node.key);
  }

  /**
   * The public key for this keypair
   */
  getPublicKey(): PublicKey {
    return new PublicKey(this.keypair.publicKey);
  }

  /**
   * Return the signature for the provided data.
   */
  signData(data: Base64DataBuffer): Base64DataBuffer {
    return new Base64DataBuffer(
      nacl.sign.detached(data.getData(), this.keypair.secretKey)
    );
  }
}
