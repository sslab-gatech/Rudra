// cargo run -- --crate-type lib samples/test.rs >stdout 2>stderr
use std::mem::ManuallyDrop;
use std::slice;

pub fn trivial() -> *const u8 {
    let local = 42;
    &local
}

fn allocate(size: usize) -> Box<[u8]> {
    unsafe {
        let mut vec = ManuallyDrop::new(Vec::with_capacity(size));
        let slice = slice::from_raw_parts_mut(vec.as_mut_ptr(), vec.capacity());
        Box::from_raw(slice)
    }
}

pub fn dangling_slice(size: usize) -> Vec<u8> {
    let mut slice = allocate(size);
    unsafe { Vec::from_raw_parts(slice.as_mut_ptr(), 0, slice.len()) }
}
