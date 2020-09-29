/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

// types without unsafe code should not be reported
pub struct NoDrop {
    vec: Vec<u32>,
}

pub struct FooSafe<'a> {
    vec: &'a mut Vec<u32>,
}

impl Drop for FooSafe<'_> {
    fn drop(&mut self) {
        println!("{}", self.vec.len());
    }
}

pub struct BarSafe {
    vec: Vec<u32>,
}

impl Drop for BarSafe {
    fn drop(&mut self) {
        println!("{}", self.vec.len());
    }
}
