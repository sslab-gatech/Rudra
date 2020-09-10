// cargo run -- --crate-type lib samples/interprocedural.rs 2>stderr

pub fn caller() {
    let raw_ptr = callee2(&mut callee1(7));

    dbg!(unsafe { raw_ptr.read() });
    unsafe {
        raw_ptr.write(2002);
    }
    dbg!(unsafe { raw_ptr.read() });
}

pub fn callee1(a: i32) -> Box<i32> {
    Box::from(a)
}

pub fn callee2(b: &mut Box<i32>) -> *mut i32 {
    &mut **b
}
