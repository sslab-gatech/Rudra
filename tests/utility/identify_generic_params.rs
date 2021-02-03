// Test case to check whether our implementation can successfully identify
// same generic parameters in multiple impl blocks with different indices.
#![allow(dead_code)]
struct My<A, B> {
    val1: A,
    val2: B,
}

unsafe impl<'a, F: Send> Send for My<i32, F> // F.index = 1
    where F: Fn(&'a u32) -> &'a u32 {}

unsafe impl<F: Sync> Sync for My<i32, F> // F.index = 0
    where F: Fn(&u32) -> &u32 {}

// unsafe impl<A: Sync, B: Sync> Sync for My<B, A> {}
    
impl<F> My<i32, F> {
    fn foo(&self) {}
}

impl<A, B> My<A, B> {
    fn bar(&self) {}
}