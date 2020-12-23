/*!
```rudra-test
test_type = "fn"
expected_analyzers = []
```
!*/

use std::fmt::Debug;

fn test_order_unsafe<I: Iterator<Item = impl Debug>>(mut iter: I) {
    unsafe {
        let _ = &*(1234 as *const i32);
    }
    println!("{:?}", iter.next());
}
