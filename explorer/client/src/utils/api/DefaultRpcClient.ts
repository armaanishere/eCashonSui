// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import {
    getExecutionStatusType,
    getTotalGasUsed,
    getTransactions,
    getTransactionDigest,
    getTransactionKindName,
    getTransferObjectTransaction,
    getTransferSuiTransaction,
    JsonRpcProvider,
} from '@mysten/sui.js';

import { deduplicate } from '../searchUtil';
import { getEndpoint, Network } from './rpcSetting';

import type {
    GetTxnDigestsResponse,
    CertifiedTransaction,
} from '@mysten/sui.js';

export { Network, getEndpoint };

const defaultRpcMap: Map<Network | string, JsonRpcProvider> = new Map();
export const DefaultRpcClient = (network: Network | string) => {
    const existingClient = defaultRpcMap.get(network);
    if(existingClient)
        return existingClient;

    const provider = new JsonRpcProvider(getEndpoint(network));
    defaultRpcMap.set(network, provider);
    return provider;
}

export const getDataOnTxDigests = (
    network: Network | string,
    transactions: GetTxnDigestsResponse
) =>
    DefaultRpcClient(network)
        .getTransactionWithEffectsBatch(deduplicate(transactions))
        .then((txEffs) => {
            return (
                txEffs
                    .map((txEff) => {
                        const [seq, digest] = transactions.filter(
                            (transactionId) =>
                                transactionId[1] ===
                                getTransactionDigest(txEff.certificate)
                        )[0];
                        const res: CertifiedTransaction = txEff.certificate;
                        // TODO: handle multiple transactions
                        const txns = getTransactions(res);
                        if (txns.length > 1) {
                            console.error(
                                'Handling multiple transactions is not yet supported',
                                txEff
                            );
                            return null;
                        }
                        const txn = txns[0];
                        const txKind = getTransactionKindName(txn);
                        const recipient =
                            getTransferObjectTransaction(txn)?.recipient ||
                            getTransferSuiTransaction(txn)?.recipient;

                        return {
                            seq,
                            txId: digest,
                            status: getExecutionStatusType(txEff),
                            txGas: getTotalGasUsed(txEff),
                            kind: txKind,
                            From: res.data.sender,
                            timestamp_ms: txEff.timestamp_ms,
                            ...(recipient
                                ? {
                                      To: recipient,
                                  }
                                : {}),
                        };
                    })
                    // Remove failed transactions and sort by sequence number
                    .filter((itm) => itm)
                    .sort((a, b) => b!.seq - a!.seq)
            );
        });
