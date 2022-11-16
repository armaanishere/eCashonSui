// A pseudo-random number generator. We use ctx (the Transaction Context) to generate
// a UID (used to create new objects), then convert that into bytes.
//
// Unlike an externally-supplied random seed, I don't believe this is suspectible to user
// manipulation; I believe the UID's generated by Sui are very dependent upon the chain state
// itself.
//
// Note that given the same ctx, you will always get the same sequence of "random" numbers.
//
// Eventually this will be replaced by a VRF from Switchboard (work in progress)

module sui::rand {
    use sui::object;
    use sui::math;
    use sui::u64;
    use sui::tx_context::TxContext;

    const EBAD_RANGE: u64 = 0;

    // Generates an integer from the range [min, max), not inclusive of max
    // bytes = vector<u8> with length == 20. However we only use the first 8 bytes
    public fun rng(min: u64, max: u64, ctx: &mut TxContext): u64 {
        assert!(max > min, EBAD_RANGE);

        let uid = object::new(ctx);
        let bytes = object::uid_to_bytes(&uid);
        object::delete(uid);

        let num = u64::from_bytes(bytes);
        math::mod(num, max - min) + min
    }
}