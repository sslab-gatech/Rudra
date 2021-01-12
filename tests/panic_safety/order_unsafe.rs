/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["UnsafeDataflow"]
```
!*/

use std::fmt::Debug;

fn test_order_unsafe<I: Iterator<Item = impl Debug>>(mut iter: I) {
    unsafe {
        std::ptr::read(1234 as *const i32);
    }
    println!("{:?}", iter.next());
}
