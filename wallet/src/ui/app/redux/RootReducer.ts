// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { combineReducers } from '@reduxjs/toolkit';

import account from './slices/account';
import app from './slices/app';
import permissions from './slices/permissions';
import suiObjects from './slices/sui-objects';
import transactionBytesRequests from './slices/transaction-bytes-requests';
import transactionRequests from './slices/transaction-requests';
import transactions from './slices/transactions';
import txresults from './slices/txresults';

const rootReducer = combineReducers({
    account,
    app,
    suiObjects,
    transactions,
    txresults,
    permissions,
    transactionRequests,
    transactionBytesRequests,
});

export type RootState = ReturnType<typeof rootReducer>;

export default rootReducer;
