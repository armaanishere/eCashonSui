// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { type CheckpointPage } from '@mysten/sui.js/src/types/checkpoints';

import { TxTableCol, TxTableHeader } from '../transactions/TxCardUtils';
import { TxTimeType } from '../tx-time/TxTimeType';

import { CheckpointLink } from '~/ui/InternalLink';
import { Text } from '~/ui/Text';

// Generate table data from the transaction data
export const genTableDataFromCheckpointsData = (data: CheckpointPage) => ({
    data: data?.data.map((checkpoint) => ({
        digest: (
            <TxTableCol isHighlightedOnHover>
                <CheckpointLink digest={checkpoint.digest} />
            </TxTableCol>
        ),
        time: (
            <TxTableCol>
                <TxTimeType timestamp={Number(checkpoint.timestampMs)} />
            </TxTableCol>
        ),
        sequenceNumber: (
            <TxTableCol>
                <Text variant="bodySmall/medium" color="steel-darker">
                    {checkpoint.sequenceNumber}
                </Text>
            </TxTableCol>
        ),
        transactionBlockCount: (
            <TxTableCol>
                <Text variant="bodySmall/medium" color="steel-darker">
                    {checkpoint.transactions.length}
                </Text>
            </TxTableCol>
        ),
    })),
    columns: [
        {
            header: () => <TxTableHeader label="Digest" />,
            accessorKey: 'digest',
        },
        {
            header: () => <TxTableHeader label="Sequence Number" />,
            accessorKey: 'sequenceNumber',
        },
        {
            header: () => <TxTableHeader label="Time" />,
            accessorKey: 'time',
        },
        {
            header: () => <TxTableHeader label="Transaction Block Count" />,
            accessorKey: 'transactionBlockCount',
        },
    ],
});
