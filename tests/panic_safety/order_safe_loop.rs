/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

use std::fmt::Debug;

fn test_order_safe_loop<I: Iterator<Item = impl Debug>>(mut iter: I) {
    for item in iter {
        unsafe {
            // `read` on `Copy` is safe.
            std::ptr::read(1234 as *const i32);
        }
    }
}
