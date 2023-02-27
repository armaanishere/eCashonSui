// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { useFeature } from '@growthbook/growthbook-react';
import {
    isValidTransactionDigest,
    isValidSuiAddress,
    isValidSuiObjectId,
    normalizeSuiObjectId,
    is,
    SuiObject,
    type JsonRpcProvider,
    getTransactionDigest,
} from '@mysten/sui.js';
import { useQuery } from '@tanstack/react-query';

import { useRpc } from '~/hooks/useRpc';
import { isGenesisLibAddress } from '~/utils/api/searchUtil';
import { GROWTHBOOK_FEATURES } from '~/utils/growthbook';

type Result = {
    label: string;
    results: { id: string; label: string; type: string }[];
};

const getResultsForTransaction = async (
    rpc: JsonRpcProvider,
    query: string
) => {
    if (!isValidTransactionDigest(query)) return null;
    try {
        const txdata = await rpc.getTransactionWithEffects(query);
        return {
            label: 'transaction',
            results: [
                {
                    id: getTransactionDigest(txdata),
                    label: getTransactionDigest(txdata),
                    type: 'transaction',
                },
            ],
        };
    } catch (e) {}
};

const getResultsForObject = async (rpc: JsonRpcProvider, query: string) => {
    const normalized = normalizeSuiObjectId(query);
    if (!isValidSuiObjectId(normalized)) return null;

    try {
        const { details, status } = await rpc.getObject(normalized);
        if (is(details, SuiObject) && status === 'Exists') {
            return {
                label: 'object',
                results: [
                    {
                        id: details.reference.objectId,
                        label: details.reference.objectId,
                        type: 'object',
                    },
                ],
            };
        }
    } catch (e) {}

    return null;
};

const getResultsForCheckpoint = async (rpc: JsonRpcProvider, query: string) => {
    try {
        const { digest } = await rpc.getCheckpoint(query);
        if (digest) {
            return {
                label: 'checkpoint',
                results: [
                    {
                        id: digest,
                        label: digest,
                        type: 'checkpoint',
                    },
                ],
            };
        }
    } catch (e) {}

    return null;
};

const getResultsForAddress = async (rpc: JsonRpcProvider, query: string) => {
    const normalized = normalizeSuiObjectId(query);
    if (!isValidSuiAddress(normalized) || isGenesisLibAddress(normalized))
        return null;

    try {
        const [from, to] = await Promise.all([
            rpc.getTransactions({ FromAddress: normalized }, null, 1),
            rpc.getTransactions({ ToAddress: normalized }, null, 1),
        ]);
        console.log(from.data.length, to.data.length);
        if (from.data?.length || to.data?.length) {
            return {
                label: 'address',
                results: [
                    {
                        id: normalized,
                        label: normalized,
                        type: 'address',
                    },
                ],
            };
        }
    } catch (e) {}

    return null;
};

export function useSearch(query: string) {
    const rpc = useRpc();
    const checkpointsEnabled = useFeature(
        GROWTHBOOK_FEATURES.EPOCHS_CHECKPOINTS
    ).on;

    return useQuery(
        ['search', query],
        async () => {
            const results = await Promise.all([
                getResultsForTransaction(rpc, query),
                ...(checkpointsEnabled
                    ? [getResultsForCheckpoint(rpc, query)]
                    : []),
                getResultsForAddress(rpc, query),
                getResultsForObject(rpc, query),
            ]);

            console.log(results);

            return results.filter(Boolean) as Result[];
        },
        {
            enabled: !!query,
            cacheTime: 10000,
        }
    );
}
