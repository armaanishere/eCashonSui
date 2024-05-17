// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import type { TransactionBlock } from '@mysten/sui/transactions';
import type { IdentifierString, WalletAccount } from '@wallet-standard/core';

/** The latest API version of the signTransactionBlock API. */
export type SuiSignTransactionBlockVersion = '1.0.0';

/**
 * @deprecated Use `sui:signTransactionBlock:v2` instead.
 *
 * A Wallet Standard feature for signing a transaction, and returning the
 * serialized transaction and transaction signature.
 */
export type SuiSignTransactionBlockFeature = {
	/** Namespace for the feature. */
	'sui:signTransactionBlock': {
		/** Version of the feature API. */
		version: SuiSignTransactionBlockVersion;
		/** @deprecated Use `sui:signTransactionBlock:v2` instead. */
		signTransactionBlock: SuiSignTransactionBlockMethod;
	};
};

/** @deprecated Use `sui:signTransactionBlock:v2` instead. */
export type SuiSignTransactionBlockMethod = (
	input: SuiSignTransactionBlockInput,
) => Promise<SuiSignTransactionBlockOutput>;

/** Input for signing transactions. */
export interface SuiSignTransactionBlockInput {
	transactionBlock: TransactionBlock;
	account: WalletAccount;
	chain: IdentifierString;
}

/** Output of signing transactions. */
export interface SuiSignTransactionBlockOutput extends SignedTransactionBlock {}

export interface SignedTransactionBlock {
	/** Transaction block as base64 encoded bcs. */
	transactionBlockBytes: string;
	/** Base64 encoded signature */
	signature: string;
}
