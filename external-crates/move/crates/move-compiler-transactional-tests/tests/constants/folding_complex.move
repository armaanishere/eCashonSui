//# run
module 0x42::m {
    const C1: u64 = {
        0 == 0;
        0 == 0;
        0 == 0;
        false == false;
        0x42 == 0x42;
        x"42" == x"42";
        b"hello" == b"hello";
        0 == 1;
        0 == 1;
        0 == 1;
        false == true;
        0x42 == 0x43;
        x"42" == x"0422";
        b"hello" == b"XhelloX";
        0 != 1;
        0 != 1;
        0 != 1;
        false != true;
        0x42 != 0x43;
        x"42" != x"0422";
        b"hello" != b"XhelloX";
        0 != 0;
        0 != 0;
        0 != 0;
        false != false;
        0x42 != 0x42;
        x"42" != x"42";
        b"hello" != b"hello";
        ((0 == 0) == (b"hello" != b"bye")) == false;
        true && true;
        true && false;
        false && true;
        false && false;
        true || true;
        true || false;
        false || true;
        false || false;
        true;
        false;
        !((true && false) || (false || true) && true) || true;
        1 << 7;
        1 << 63;
        (1: u128) << 127;
        128 >> 7;
        18446744073709551615 >> 63;
        (340282366920938463463374607431768211455: u128) >> 127;
        255 / 2;
        18446744073709551615 / 2;
        (340282366920938463463374607431768211455: u128) / 2;
        255 % 2;
        18446744073709551615 % 2;
        (340282366920938463463374607431768211455: u128) % 2;
        254 + 1;
        18446744073709551614 + 1;
        (340282366920938463463374607431768211454: u128) + 1;
        255 - 255;
        18446744073709551615 - 18446744073709551615;
        (340282366920938463463374607431768211455: u128) - (340282366920938463463374607431768211455: u128);
        ((255: u64) as u8);
        ((18446744073709551615: u128) as u64);
        ((1: u8) as u128);
        255 & 255;
        18446744073709551615 & 18446744073709551615;
        (340282366920938463463374607431768211455: u128) & (340282366920938463463374607431768211455: u128);
        255 | 255;
        18446744073709551615 | 18446744073709551615;
        (340282366920938463463374607431768211455: u128) | (340282366920938463463374607431768211455: u128);
        255 ^ 255;
        18446744073709551615 ^ 18446744073709551615;
        (340282366920938463463374607431768211455: u128) ^ (340282366920938463463374607431768211455: u128);
        ((3402823669209384: u256) as u128);
        (340282366920938463463374607431768211455340282366920938463463374607: u256) - 34;
        (340282366920938463463374607431768211455340282366920938463463374607: u256) - (340282366920938463463374607431768211455340: u256);
        (340282366920938463463374607431768211455340282366920938463463374607: u256) ^ (340282366920938463463374607431768211455340: u256);
        (340282366920938463463374607431768211455340282366920938463463374607: u256) ^ (340282366920938463463374607431768211455: u256);
        0
    };
    const C2: bool = {
        ();
        ();
        (true && true) &&
        (!false) &&
        (1 << 7 == 128) &&
        (128 >> 7 == 1) &&
        (255 / 2 == 127) &&
        (255 % 2 == 1) &&
        (254 + 1 == 255) &&
        (255 - 255 == 0) &&
        (255 & 255 == 255) &&
        (255 | 255 == 255) &&
        (255 ^ 255 == 0) &&
        (x"42" == x"42") &&
        (b"hello" != b"bye")
    };

    fun main() {
        assert!(C1 == 0, 41);
        assert!(C2, 42)
    }
}
