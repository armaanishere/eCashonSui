// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';
import FindKiosk from '../Kiosk/FindKiosk';
import { SuiConnectButton } from './SuiConnectButton';

export function Header(): JSX.Element {
  const navigate = useNavigate();

  return (
    <div className="border-b border-gray-400">
      <div className="md:flex items-center gap-2 container py-4 ">
        <button
          className="text-lg font-bold text-center mr-3 bg-transparent"
          onClick={() => navigate('/')}
        >
          Kiosk demo
        </button>
        <button className="mr-2 bg-transparent" onClick={() => navigate('/')}>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="22"
            height="22"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path>
            <polyline points="9 22 9 12 15 12 15 22"></polyline>
          </svg>
        </button>
        <FindKiosk />
        <div className="ml-auto my-3 md:my-1">
          <SuiConnectButton></SuiConnectButton>
        </div>
      </div>
    </div>
  );
}
