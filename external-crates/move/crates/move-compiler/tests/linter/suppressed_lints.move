module 0x42::M {

    #[allow(lint(constant_naming))]
    const Another_BadName: u64 = 42; // Should trigger a warning

    #[allow(lint(absurd_extreme_comparisons))]
    fun func1(x: u64) {
        let u64_max: u64 = 18446744073709551615;
        let u64_min = 0;
        if (x > u64_max){

        };

        if (x < u64_min){

        };
    }
}
