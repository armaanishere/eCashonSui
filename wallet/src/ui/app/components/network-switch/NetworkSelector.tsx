// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import cl from 'classnames';
import { useMemo, useCallback } from 'react';

import { API_ENV_TO_INFO, API_ENV } from '_app/ApiProvider';
import BsIcon from '_components/bs-icon';
import { useAppSelector, useAppDispatch } from '_hooks';
import {
    setApiEnv,
    changeRPCNetwork,
    setNetworkSelector,
} from '_redux/slices/app';
import { getTransactionsByAddress } from '_redux/slices/txresults';
import store from '_store';

import st from './Network.module.scss';

const NetworkSelector = () => {
    const selectedApiEnv = useAppSelector(({ app }) => app.apiEnv);
    const dispatch = useAppDispatch();
    const networkList: any = API_ENV_TO_INFO;
    const netWorks = useMemo(
        () =>
            Object.keys(networkList).map((itm: string) => ({
                style: { color: networkList[itm].color },
                ...networkList[itm],
                networkName: itm,
            })),
        [networkList]
    );

    const changeNetwork = useCallback(
        (networkName: string) => () => {
            const apiEnv = API_ENV[networkName as keyof typeof API_ENV];
            dispatch(setNetworkSelector(true));
            store.dispatch(setApiEnv(apiEnv));
            dispatch(changeRPCNetwork()).unwrap();
            dispatch(getTransactionsByAddress()).unwrap();
        },
        [dispatch]
    );

    return (
        <div className={st['network-options']}>
            <div className={st['network-header']}>RPC NETWORK</div>
            <ul className={st['network-lists']}>
                {netWorks.map((apiEnv, index) => (
                    <li
                        className={st['network-item']}
                        key={`networkid-${index}`}
                        onClick={changeNetwork(apiEnv.networkName)}
                    >
                        <BsIcon
                            icon="check2"
                            className={cl(
                                st['selected-network'],
                                selectedApiEnv === apiEnv.networkName
                                    ? st['network-active']
                                    : ''
                            )}
                        />
                        <div style={apiEnv.style}>
                            <BsIcon
                                icon="circle-fill"
                                className={st['network-icon']}
                            />
                        </div>
                        {apiEnv.name}
                    </li>
                ))}

                <li className={st['network-item']}>
                    <BsIcon
                        icon="check2"
                        className={cl(st['selected-network'])}
                    />
                    <BsIcon icon="circle-fill" className={st['network-icon']} />
                    Custom
                </li>
            </ul>
        </div>
    );
};

export default NetworkSelector;
