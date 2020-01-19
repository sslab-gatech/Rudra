// cargo run -- --crate-type lib samples/test.rs 2>stderr

pub fn crux_test_trivial_escape() -> *const u8 {
    let local = 42;
    &local
}

pub fn crux_test_tmp_var_uaf() {
    use std::ffi::CString;
    let ptr = CString::new("Hello, world!").unwrap().as_ptr();
    println!("First byte of the ptr is: {}", unsafe { *ptr });
}

#[derive(Debug)]
struct MyStruct {
    x: i32,
    y: i32,
}

pub fn crux_test_struct() {
    dbg!(MyStruct { x: 1, y: 2 });
}
