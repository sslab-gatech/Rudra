/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["UnsafeDataflow"]
```
!*/

pub struct MyVec<T>(Vec<T>);

impl<T: Clone> MyVec<T> {
    // Example from: https://doc.rust-lang.org/nomicon/exception-safety.html#vecpush_all
    fn push_all(&mut self, to_push: &[T]) {
        self.0.reserve(to_push.len());
        unsafe {
            // can't overflow because we just reserved this
            self.0.set_len(self.0.len() + to_push.len());

            for (i, x) in to_push.iter().enumerate() {
                // Clone might panic
                self.0.as_mut_ptr().offset(i as isize).write(x.clone());
            }
        }
    }
}
