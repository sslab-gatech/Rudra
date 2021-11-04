/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/
#![feature(negative_impls)]

struct Negative<T>(T);

impl<T> !Send for Negative<T> {}
impl<T> !Sync for Negative<T> {}
