//# run
module 0x42::m {
fun main() {
    if (true) {
        return ()
    } else {
        assert!(false, 42);
    };
    assert!(false, 43);
}
}
