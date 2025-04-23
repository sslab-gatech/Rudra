/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["UnsafeDataflow"]
```
!*/

use std::ptr;
fn unaligned_insertion_sort_unsafe<T: Ord>(arr: &mut [T]) {
    unsafe {
        for i in 1..arr.len() {
            let item = ptr::read_unaligned(&arr[i]);
            let mut j = i;
            while j > 0 && arr[j - 1] > item {
                ptr::swap(arr.as_mut_ptr().add(j), arr.as_mut_ptr().add(j - 1));
                j -= 1;
            }
            ptr::write_unaligned(arr.as_mut_ptr().add(j), item);
        }
    }
}
