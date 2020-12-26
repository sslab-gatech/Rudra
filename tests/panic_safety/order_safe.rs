/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

use std::fmt::Debug;

fn test_order_safe<I: Iterator<Item = impl Debug>>(mut iter: I) {
    println!("{:?}", iter.next());
    unsafe {
        std::ptr::read(1234 as *const i32);
    }
}
