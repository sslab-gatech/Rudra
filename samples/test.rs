// cargo run -- --crate-type lib samples/test.rs 2>stderr

pub fn trivial() -> *const u8 {
    let local = 42;
    &local
}
