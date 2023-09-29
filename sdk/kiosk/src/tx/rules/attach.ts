// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@mysten/sui.js/bcs';
import { type TransactionBlock } from '@mysten/sui.js/transactions';

import { type ObjectArgument } from '../../types';
import { objArg } from '../../utils';

export function attachKioskLockRuleTx(
	tx: TransactionBlock,
	type: string,
	policy: ObjectArgument,
	policyCap: ObjectArgument,
	packageId: string,
) {
	tx.moveCall({
		target: `${packageId}::kiosk_lock_rule::add`,
		typeArguments: [type],
		arguments: [objArg(tx, policy), objArg(tx, policyCap)],
	});
}

export function attachRoyaltyRuleTx(
	tx: TransactionBlock,
	type: string,
	policy: ObjectArgument,
	policyCap: ObjectArgument,
	percentageBps: number | string, // this is in basis points.
	minAmount: number | string,
	packageId: string,
) {
	if (Number(percentageBps) < 0 || Number(percentageBps) > 10_000)
		throw new Error('Invalid basis point percentage. Use a value between [0,10000].');

	tx.moveCall({
		target: `${packageId}::royalty_rule::add`,
		typeArguments: [type],
		arguments: [
			objArg(tx, policy),
			objArg(tx, policyCap),
			tx.pure.u16(Number(percentageBps)),
			tx.pure.u64(minAmount),
		],
	});
}

export function attachPersonalKioskRuleTx(
	tx: TransactionBlock,
	type: string,
	policy: ObjectArgument,
	policyCap: ObjectArgument,
	packageId: string,
) {
	tx.moveCall({
		target: `${packageId}::personal_kiosk_rule::add`,
		typeArguments: [type],
		arguments: [objArg(tx, policy), objArg(tx, policyCap)],
	});
}

export function attachFloorPriceRuleTx(
	tx: TransactionBlock,
	type: string,
	policy: ObjectArgument,
	policyCap: ObjectArgument,
	minPrice: string | bigint,
	packageId: string,
) {
	tx.moveCall({
		target: `${packageId}::floor_price_rule::add`,
		typeArguments: [type],
		arguments: [objArg(tx, policy), objArg(tx, policyCap), tx.pure.u64(minPrice)],
	});
}
