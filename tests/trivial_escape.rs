// cargo run -- --crate-type lib samples/trivial_escape.rs 2>stderr

pub fn crux_test_trivial_escape() -> *const u8 {
    let local = 42;
    &local
}
