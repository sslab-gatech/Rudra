/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["PanicSafety"]
```
!*/

use std::fmt::Debug;

fn test_order_unsafe_loop<I: Iterator<Item = impl Debug>>(mut iter: I) {
    for item in iter {
        unsafe {
            std::ptr::read(1234 as *const i32);
        }
    }
}
