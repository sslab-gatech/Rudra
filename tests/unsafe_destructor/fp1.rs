/*!
```crux-test
test_type = "fp"
expected_analyzers = ["UnsafeDestructor"]
```
!*/

pub struct Leak<'a> {
    vec: &'a mut Vec<u32>,
}

// calling an actual unsafe function, needs developer triage
// this case, memory is leaked but it is not UB
impl Drop for Leak<'_> {
    fn drop(&mut self) {
        unsafe {
            self.vec.set_len(0);
        }
    }
}
