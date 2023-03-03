// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_src/ui/app/shared/ButtonUI';
import { Link } from '_src/ui/app/shared/Link';
import { ModalDialog } from '_src/ui/app/shared/ModalDialog';
import { Text } from '_src/ui/app/shared/text';

type ConnectLedgerModalProps = {
    isOpen: boolean;
    onClose: () => void;
    onConfirm: () => void;
};

export function ConnectLedgerModal({
    isOpen,
    onClose,
    onConfirm,
}: ConnectLedgerModalProps) {
    return (
        <ModalDialog
            isOpen={isOpen}
            title="Connect Ledger Wallet"
            onClose={onClose}
            body={
                <>
                    <LedgerLogo />
                    <div className="break-words text-center">
                        <Text variant="p2" color="steel-dark" weight="normal">
                            Connect your ledger to your computer, unlock it, and
                            launch the Sui app. Click Continue when done.
                        </Text>
                        <div className="mt-3">
                            <Text
                                variant="p2"
                                color="steel-dark"
                                weight="normal"
                            >
                                <span>Need more help?</span>{' '}
                                <Link
                                    href="https://mystenlabs.com"
                                    text="View tutorial."
                                    color="suiDark"
                                    weight="medium"
                                />
                            </Text>
                        </div>
                    </div>
                </>
            }
            footer={
                <div className="flex flex-row self-center gap-3">
                    <div>
                        <Button
                            variant="outline"
                            text="Cancel"
                            onClick={onClose}
                        />
                    </div>
                    <div>
                        <Button
                            variant="outline"
                            text="Continue"
                            onClick={onConfirm}
                        />
                    </div>
                </div>
            }
        />
    );
}

function LedgerLogo() {
    return (
        <svg
            width="383"
            height="128"
            viewBox="0 0 383 128"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
        >
            <path
                d="M327.629 119.94V127.998H382.998V91.6548H374.931V119.94H327.629ZM327.629 0V8.05844H374.931V36.3452H382.998V0H327.629ZM299.075 62.3411V43.6158H311.731C317.901 43.6158 320.116 45.6696 320.116 51.2803V54.5982C320.116 60.3657 317.98 62.3411 311.731 62.3411H299.075ZM319.165 65.6589C324.939 64.1578 328.972 58.7842 328.972 52.3856C328.972 48.3564 327.391 44.7211 324.385 41.7972C320.589 38.1619 315.525 36.3452 308.961 36.3452H291.164V91.6529H299.075V69.6097H310.94C317.03 69.6097 319.483 72.1378 319.483 78.4599V91.6548H327.55V79.7239C327.55 71.0325 325.494 67.7147 319.165 66.7662V65.6589ZM252.565 67.4756H276.928V60.207H252.565V43.6139H279.3V36.3452H244.496V91.6529H280.487V84.3842H252.565V67.4756ZM226.065 70.3995V74.1916C226.065 82.1717 223.138 84.78 215.783 84.78H214.043C206.685 84.78 203.126 82.4088 203.126 71.4264V56.5717C203.126 45.5109 206.844 43.2181 214.2 43.2181H215.781C222.979 43.2181 225.273 45.9048 225.351 53.3322H234.052C233.262 42.4283 225.985 35.5555 215.069 35.5555C209.77 35.5555 205.34 37.2153 202.018 40.3745C197.035 45.0367 194.266 52.9383 194.266 63.9991C194.266 74.6659 196.64 82.5675 201.543 87.4649C204.865 90.7044 209.454 92.4426 213.962 92.4426C218.708 92.4426 223.06 90.5456 225.273 86.438H226.379V91.6529H233.656V63.1309H212.22V70.3995H226.065ZM156.301 43.6139H164.924C173.072 43.6139 177.502 45.6677 177.502 56.7304V71.2677C177.502 82.3285 173.072 84.3842 164.924 84.3842H156.301V43.6139ZM165.634 91.6548C180.743 91.6548 186.358 80.1982 186.358 64.001C186.358 47.5666 180.346 36.3471 165.475 36.3471H148.389V91.6548H165.634ZM110.186 67.4756H134.549V60.207H110.186V43.6139H136.921V36.3452H102.116V91.6529H138.108V84.3842H110.186V67.4756ZM63.5175 36.3452H55.45V91.6529H91.8359V84.3842H63.5175V36.3452ZM0 91.6548V128H55.3696V119.94H8.06747V91.6548H0ZM0 0V36.3452H8.06747V8.05844H55.3696V0H0Z"
                fill="black"
            />
        </svg>
    );
}
