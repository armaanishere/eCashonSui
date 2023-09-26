// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';

export const triggerButton = style({
	display: 'inline-flex',
	justifyContent: 'space-between',
	alignItems: 'center',
	gap: 8,
	paddingLeft: 16,
	paddingRight: 16,
	paddingTop: 24,
	paddingBottom: 24,
	borderRadius: 12,
	boxShadow: '0px 4px 12px rgba(0, 0, 0, 0.1)',
	backgroundColor: 'white',
	color: '#182435',
});

export const menuContent = style({
	width: 180,
	maxHeight: 200,
	borderRadius: 12,
	marginTop: 4,
	color: 'red',
	boxShadow: 'k',
	padding: 8,
	display: 'flex',
	flexDirection: 'column',
	gap: 8,
	backgroundColor: 'white',
});

export const menuItem = style({
	color: 'RED',
	display: 'flex',
	justifyContent: 'space-between',
	alignItems: 'center',
});

export const separator = style({
	height: 1,
	backgroundColor: '#F3F6F8',
});
