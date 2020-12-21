/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

use std::fmt::Debug;

fn test_order_safe_if<I: Iterator<Item = impl Debug>>(mut iter: I) {
    if true {
        unsafe {
            std::ptr::read(1234 as *const i32);
        }
    } else {
        println!("{:?}", iter.next());
    }
}
