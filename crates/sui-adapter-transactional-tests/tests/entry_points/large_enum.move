// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// tests error after serializing a large enum return value

//# init --addresses test=0x0 --accounts A

//# publish

module test::m {

public enum X1 {
    Big(u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8),
}

public enum X2 {
    V1(X1, X1, X1),
    V2(X1, X1, X1),
    V3(X1, X1, X1),
}

public enum X3 {
    X2(X2, X2, X2),
    U64(u64),
}

entry fun x1(): X1 {
    X1::Big(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)
}

entry fun x3(): X3 {
    X3::U64(0)
}

}


//# programmable --sender A
//> test::m::x1()
