// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { SUI_TYPE_ARG } from '@mysten/sui.js';
import { ErrorMessage, Field, Form, useFormikContext } from 'formik';
import { useRef, memo, useCallback, useMemo } from 'react';

import { Content } from '_app/shared/bottom-menu-layout';
import { Card } from '_app/shared/card';
import { Text } from '_app/shared/text';
import Alert from '_components/alert';
import NumberInput from '_components/number-input';
import { parseAmount } from '_helpers';
import { useFormatCoin } from '_hooks';
import { DEFAULT_GAS_BUDGET_FOR_STAKE } from '_redux/slices/sui-objects/Coin';

import type { FormValues } from './StakingCard';

const HIDE_MAX = true;

export type StakeFromProps = {
    submitError: string | null;
    coinBalance: bigint;
    coinType: string;
    coinDecimals: number;
    epoch: string;
    onClearSubmitError: () => void;
};

function AvailableBalance({ amount }: { amount: bigint }) {
    const [formatted, symbol] = useFormatCoin(amount, SUI_TYPE_ARG);
    return (
        <Text variant="bodySmall" color="steel" weight="medium">
            Available - {+formatted > 0 ? formatted : 0} {symbol}
        </Text>
    );
}

function StakeForm({
    submitError,
    coinBalance,
    coinType,
    onClearSubmitError,
    coinDecimals,
    epoch,
}: StakeFromProps) {
    const {
        setFieldValue,
        values: { amount },
    } = useFormikContext<FormValues>();

    const onClearRef = useRef(onClearSubmitError);
    onClearRef.current = onClearSubmitError;
    const gasBudget = BigInt(
        coinType === SUI_TYPE_ARG ? DEFAULT_GAS_BUDGET_FOR_STAKE : 0
    );

    const [gasBudgetEstimation] = useFormatCoin(
        DEFAULT_GAS_BUDGET_FOR_STAKE,
        SUI_TYPE_ARG
    );

    const coinBalanceMinusGas = coinBalance - gasBudget;

    const [maxToken, symbol, queryResult] = useFormatCoin(
        coinBalanceMinusGas,
        coinType
    );

    const setMaxToken = useCallback(() => {
        if (!maxToken) return;
        setFieldValue('amount', maxToken);
    }, [maxToken, setFieldValue]);

    const calculateRemaining = useMemo(() => {
        if (!coinBalance) return 0n;
        const bigIntAmount = parseAmount(amount, coinDecimals);
        return coinBalance - bigIntAmount - gasBudget;
    }, [amount, coinBalance, coinDecimals, gasBudget]);

    return (
        <Form className="flex flex-1 flex-col flex-nowrap" autoComplete="off">
            <Content>
                <div className="flex flex-col justify-between items-center mb-2 mt-3.5 w-full gap-1.5">
                    <Text variant="caption" color="gray-85" weight="semibold">
                        Enter the amount of SUI to stake
                    </Text>
                    <AvailableBalance amount={calculateRemaining} />
                </div>
                <Card
                    variant="gray"
                    titleDivider
                    header={
                        <div className="p-2.5 w-full flex bg-white">
                            <Field
                                component={NumberInput}
                                allowNegative={false}
                                name="amount"
                                className="w-full border-none text-hero-dark text-heading4 font-semibold bg-white placeholder:text-gray-70 placeholder:font-semibold"
                                decimals
                            />
                            {!HIDE_MAX ? (
                                <button
                                    className="bg-white border border-solid border-gray-60 hover:border-steel-dark rounded-2xl h-6 w-11 flex justify-center items-center cursor-pointer text-steel-darker hover:text-steel-darker text-bodySmall font-medium disabled:opacity-50 disabled:cursor-auto"
                                    onClick={setMaxToken}
                                    disabled={queryResult.isLoading}
                                    type="button"
                                >
                                    Max
                                </button>
                            ) : null}
                        </div>
                    }
                    footer={
                        <div className="py-px flex justify-between w-full">
                            <Text
                                variant="body"
                                weight="medium"
                                color="steel-darker"
                            >
                                Gas Fees
                            </Text>
                            <Text
                                variant="body"
                                weight="medium"
                                color="steel-darker"
                            >
                                {gasBudgetEstimation} {symbol}
                            </Text>
                        </div>
                    }
                >
                    <div className="pb-3.75 flex justify-between w-full">
                        <Text
                            variant="body"
                            weight="medium"
                            color="steel-darker"
                        >
                            Staking Rewards Start
                        </Text>
                        <Text
                            variant="body"
                            weight="medium"
                            color="steel-darker"
                        >
                            Epoch #{+epoch + 2}
                        </Text>
                    </div>
                </Card>
                <ErrorMessage name="amount" component="div">
                    {(msg) => (
                        <div className="mt-2 flex flex-col flex-nowrap">
                            <Alert mode="warning" className="text-body">
                                {msg}
                            </Alert>
                        </div>
                    )}
                </ErrorMessage>

                {submitError ? (
                    <div className="mt-2 flex flex-col flex-nowrap">
                        <Alert mode="warning">
                            <strong>Stake failed</strong>
                            <small>{submitError}</small>
                        </Alert>
                    </div>
                ) : null}
            </Content>
        </Form>
    );
}

export default memo(StakeForm);
