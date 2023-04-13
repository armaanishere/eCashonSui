// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { useRpcClient } from '@mysten/core';
import { useInfiniteQuery } from '@tanstack/react-query';

import type { SuiAddress } from '@mysten/sui.js';

export const DEFAULT_TRANSACTIONS_LIMIT = 100;

// Fetch transaction blocks for an address, w/ toggle for to/from filter
export function useGetTransactionBlocks(
    address: SuiAddress,
    isFrom?: boolean,
    limit = DEFAULT_TRANSACTIONS_LIMIT
) {
    const rpc = useRpcClient();
    const filter = isFrom ? { FromAddress: address } : { ToAddress: address };

    return useInfiniteQuery(
        ['get-transaction-blocks', address, isFrom],
        async ({ pageParam }) =>
            await rpc.queryTransactionBlocks({
                cursor: pageParam ? pageParam.cursor : null,
                filter,
                order: 'descending',
                limit,
                options: {
                    showEffects: true,
                    showBalanceChanges: true,
                    showInput: true,
                },
            }),
        {
            getNextPageParam: (lastPage) =>
                lastPage?.hasNextPage
                    ? {
                          cursor: lastPage.nextCursor,
                      }
                    : false,
            enabled: !!address,
        }
    );
}
