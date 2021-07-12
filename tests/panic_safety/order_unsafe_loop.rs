/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["UnsafeDataflow"]
```
!*/

use std::fmt::Debug;

fn test_order_unsafe_loop<I: Iterator<Item = impl Debug>>(mut iter: I) {
    // Non-Copy type
    let non_copy = Box::new(1234);
    for item in iter {
        unsafe {
            std::ptr::read(&non_copy);
        }
    }
}
