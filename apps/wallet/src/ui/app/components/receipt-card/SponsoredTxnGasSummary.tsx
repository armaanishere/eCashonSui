// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { type SuiAddress } from '@mysten/sui.js';

import { TxnAddressLink } from './TxnAddressLink';
import { useFormatCoin } from '_hooks';
import { GAS_TYPE_ARG } from '_redux/slices/sui-objects/Coin';
import { Text } from '_src/ui/app/shared/text';

type SponsoredTxnGasSummaryProps = {
    totalGas: number;
    sponsor: SuiAddress;
};

export function SponsoredTxnGasSummary({
    totalGas,
    sponsor,
}: SponsoredTxnGasSummaryProps) {
    const [senderTotalAmount, senderTotalAmountSymbol] = useFormatCoin(
        0,
        GAS_TYPE_ARG
    );
    const [sponsorTotalAmount, sponsorTotalAmountSymbol] = useFormatCoin(
        totalGas,
        GAS_TYPE_ARG
    );

    return (
        <div className="flex flex-col w-full gap-3.5 border-t border-solid border-steel/20 border-x-0 border-b-0 py-3.5 first:pt-0">
            <Text variant="body" weight="medium" color="steel">
                Gas Fees
            </Text>
            <div className="flex justify-between items-center w-full">
                <Text variant="body" weight="medium" color="steel-darker">
                    You Paid
                </Text>
                <div className="flex gap-1 items-center">
                    <Text variant="body" weight="medium" color="steel-darker">
                        {senderTotalAmount} {senderTotalAmountSymbol}
                    </Text>
                </div>
            </div>
            <div className="flex justify-between items-center w-full">
                <Text variant="body" weight="medium" color="steel-darker">
                    Paid by Sponsor
                </Text>
                <div className="flex gap-1 items-center">
                    <Text variant="body" weight="medium" color="steel-darker">
                        {sponsorTotalAmount} {sponsorTotalAmountSymbol}
                    </Text>
                </div>
            </div>
            <div className="flex justify-between items-center w-full">
                <Text variant="body" weight="medium" color="steel-darker">
                    Sponsor
                </Text>
                <div className="flex gap-1 items-center">
                    <TxnAddressLink address={sponsor} />
                </div>
            </div>
        </div>
    );
}
