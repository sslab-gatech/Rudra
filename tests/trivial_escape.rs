// cargo run -- --crate-type lib samples/trivial_escape.rs 2>stderr

pub fn rudra_test_trivial_escape() -> *const u8 {
    let local = 42;
    &local
}
