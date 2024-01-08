// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import type { Output } from 'valibot';
import { literal, object, string, union, uuid } from 'valibot';

export type ZkSendSignPersonalMessageResponse = Output<typeof ZkSendSignPersonalMessageResponse>;

export const ZkSendRejectResponse = object({
	type: literal('reject'),
});

export const ZdSendConnectResponse = object({
	address: string(),
});

export const ZkSendSignTransactionBlockResponse = object({
	signature: string(),
});

export const ZkSendSignPersonalMessageResponse = object({
	signature: string(),
});

export interface ZkSendRequestTypes extends Record<string, Record<string, string>> {
	// eslint-disable-next-line @typescript-eslint/ban-types
	connect: {};
	'sign-transaction-block': {
		bytes: string;
		address: string;
	};
	'sign-personal-message': {
		bytes: string;
		address: string;
	};
}

export type ZkSendResponseTypes = {
	connect: Output<typeof ZdSendConnectResponse>;
	'sign-transaction-block': Output<typeof ZkSendSignTransactionBlockResponse>;
	'sign-personal-message': Output<typeof ZkSendSignPersonalMessageResponse>;
};

export const ZkSendResponseData = union([
	ZdSendConnectResponse,
	ZkSendSignTransactionBlockResponse,
	ZkSendSignPersonalMessageResponse,
]);

export const ZkSendResolveResponse = object({
	type: literal('resolve'),
	data: ZkSendResponseData,
});

export type ZkSendResolveResponse = Output<typeof ZkSendResolveResponse>;

export const ZkSendResponsePayload = union([ZkSendRejectResponse, ZkSendResolveResponse]);
export type ZkSendResponsePayload = Output<typeof ZkSendResponsePayload>;

export const ZkSendResponse = object({
	id: string([uuid()]),
	source: literal('zksend-channel'),
	payload: ZkSendResponsePayload,
});

export type ZkSendResponse = Output<typeof ZkSendResponse>;
