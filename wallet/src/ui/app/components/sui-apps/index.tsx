// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import cl from 'classnames';
import { useCallback, useEffect, useState } from 'react';

import SuiApp, { SuiAppEmpty } from './SuiApp';
import Button from '_app/shared/button';
import ExplorerLink from '_components/explorer-link';
import { ExplorerLinkType } from '_components/explorer-link/ExplorerLinkType';
import Icon, { SuiIcons } from '_components/icon';
import LoadingIndicator from '_components/loading/LoadingIndicator';
import { useAppSelector, useAppDispatch } from '_hooks';
import { getCuratedApps } from '_redux/slices/dapps';
import { mintDemoNFT } from '_redux/slices/sui-objects';

import type { SerializedError } from '@reduxjs/toolkit';

import st from './Playground.module.scss';

function AppsPlayGround() {
    const [mintInProgress, setMintInProgress] = useState(false);
    const [mintStatus, setMintStatus] = useState<boolean | null>(null);
    const [mintError, setMintError] = useState<string | null>(null);
    const dispatch = useAppDispatch();

    useEffect(() => {
        dispatch(getCuratedApps()).unwrap();
        let timeout: number;
        if (mintStatus !== null) {
            timeout = window.setTimeout(() => setMintStatus(null), 3000);
        }
        return () => {
            if (timeout) {
                clearTimeout(timeout);
            }
        };
    }, [dispatch, mintStatus]);

    const curatedDapps = useAppSelector(
        ({ curatedApps }) => curatedApps.curatedApps
    );

    const handleMint = useCallback(async () => {
        setMintInProgress(true);
        setMintError(null);
        try {
            await dispatch(mintDemoNFT()).unwrap();
            setMintStatus(true);
        } catch (e) {
            setMintStatus(false);
            setMintError((e as SerializedError).message || null);
        } finally {
            setMintInProgress(false);
        }
    }, [dispatch]);

    const mintStatusIcon =
        mintStatus !== null ? (mintStatus ? 'check2' : 'x-lg') : null;

    return (
        <div className={cl(st.container)}>
            <h4 className={st.activeSectionTitle}>Playground</h4>
            <div className={st.groupButtons}>
                <Button
                    size="large"
                    mode="outline"
                    className={cl('btn', st.cta, st['mint-btn'])}
                    onClick={handleMint}
                    disabled={mintInProgress || mintStatus !== null}
                >
                    {mintInProgress ? <LoadingIndicator /> : 'Mint an NFT'}

                    {mintStatusIcon ? (
                        <Icon
                            icon={mintStatusIcon}
                            className={cl(st['mint-icon'], {
                                [st.success]: mintStatus,
                                [st.fail]: !mintStatus,
                            })}
                        />
                    ) : (
                        <Icon
                            icon={SuiIcons.ArrowRight}
                            className={cl(st.arrowActionIcon, st.angledArrow)}
                        />
                    )}
                </Button>

                <ExplorerLink
                    className={cl('btn', st.cta, st.outline)}
                    type={ExplorerLinkType.address}
                    useActiveAddress={true}
                    showIcon={false}
                >
                    View account on Sui Explorer
                    <Icon
                        icon={SuiIcons.ArrowRight}
                        className={cl(st.arrowActionIcon, st.angledArrow)}
                    />
                </ExplorerLink>
                {mintError ? (
                    <div className={st.error}>
                        <strong>Minting NFT failed.</strong>
                        <div>
                            <small>{mintError}</small>
                        </div>
                    </div>
                ) : null}
            </div>
            {curatedDapps && curatedDapps.length ? (
                <>
                    <div className={st.desc}>
                        <div className={st.title}>
                            Builders in sui ecosystem
                        </div>
                        Apps here are actively curated but do not indicate any
                        endorsement or relationship with Sui Wallet. Please
                        DYOR.
                    </div>

                    <div className={st.apps}>
                        {curatedDapps.map((app, index) => (
                            <SuiApp key={index} {...app} displaytype="full" />
                        ))}
                    </div>
                </>
            ) : (
                <>
                    <div className={st.desc}>
                        <div className={st.title}>
                            Builders in sui ecosystem
                        </div>
                        Something went wrong
                    </div>

                    <SuiAppEmpty displaytype="full" />
                </>
            )}
        </div>
    );
}

export default AppsPlayGround;
export { default as ConnectedAppsCard } from './ConnectedAppsCard';
export { default as AppFiltersPortal } from './filters';
