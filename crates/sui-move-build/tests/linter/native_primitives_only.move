// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

module 0x42::test1 {
    //has copy needed because it's a vector

    struct S1 has copy{}
    
    struct Coolstruct has copy ,drop {
        a: bool,
        b: u64,
    }

    #[allow(unused_function)]
    native fun returns_something(a:bool,b:u64,c:Coolstruct,d:&Coolstruct) : (bool,u64);

    public entry fun main(){
        let (_x,_y) = returns_something(true,42,Coolstruct{a:true,b:42},&Coolstruct{a:true,b:42});
    }
}
