#![allow(dead_code)]

// B.index = 1
struct My<A, B> {
    val1: A,
    val2: B,
}

// B.index = 2
// By using `generic_param_idx_map`, we can retrieve the original index 1.
unsafe impl<'a, A: 'a + Send, B: Sync> Sync for My<A, B>
    where B: Fn(&'a A)
{}

impl<'a, A: 'a + Send, B: Sync> My<A, B>
    where B: Fn(&'a A)
{
    // C.index = 3
    pub fn hello<'b, C>(&self, x: C, y: &'b B) {}
}
