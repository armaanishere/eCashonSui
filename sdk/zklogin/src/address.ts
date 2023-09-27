// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { computeZkLoginAddressFromSeed } from '@mysten/sui.js/zklogin';
import { genAddressSeed } from './utils.js';
import { base64url, decodeJwt } from 'jose';
import { JSONProcessor, lengthChecks } from './checks';

export function jwtToAddress(jwt: string, userSalt: bigint) {
	const decodedJWT = decodeJwt(jwt);
	if (!decodedJWT.iss) {
		throw new Error('Missing iss');
	}

	const keyClaimName = 'sub';
	const [header, payload] = jwt.split('.');
	const decoded_payload = base64url.decode(payload).toString();
	const processor = new JSONProcessor(decoded_payload);
	const keyClaimDetails = processor.process(keyClaimName); // throws an error if key claim name is not found
	if (typeof keyClaimDetails.value !== 'string') {
		throw new Error('Key claim value must be a string');
	}
	const audDetails = processor.process('aud');
	if (typeof audDetails.value !== 'string') {
		throw new Error('Aud claim value must be a string');
	}

	lengthChecks(header, payload, keyClaimName, processor);

	return computeZkAddress({
		userSalt,
		claimName: keyClaimName,
		claimValue: keyClaimDetails.value,
		aud: audDetails.value,
		iss: decodedJWT.iss,
	});
}

export interface ComputeZKAddressOptions {
	claimName: string;
	claimValue: string;
	userSalt: bigint;
	iss: string;
	aud: string;
}

export function computeZkAddress({
	claimName,
	claimValue,
	iss,
	aud,
	userSalt,
}: ComputeZKAddressOptions) {
	return computeZkLoginAddressFromSeed(genAddressSeed(userSalt, claimName, claimValue, aud), iss);
}
