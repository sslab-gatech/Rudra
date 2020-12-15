/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["PanicSafety"]
```
!*/

use std::ptr;

pub struct Dummy;

impl Dummy {
    fn insertion_sort_unsafe<T: Ord>(arr: &mut [T]) {
        unsafe {
            for i in 1..arr.len() {
                let item = ptr::read(&arr[i]);
                let mut j = i - 1;
                while j >= 0 && arr[j] > item {
                    j = j - 1;
                }
                ptr::copy(&mut arr[j + 1], &mut arr[j + 2], i - j - 1);
                ptr::write(&mut arr[j + 1], item);
            }
        }
    }
}
