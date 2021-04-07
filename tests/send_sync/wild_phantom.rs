/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncVariance"]
```
!*/

use std::marker::PhantomData;
use std::ptr::NonNull;

struct Atom1<'a, T> {
    ptr: NonNull<T>,
    _marker1: PhantomData<&'a mut T>,
}
unsafe impl<'a, A> Send for Atom1<'a, A> {}
unsafe impl<'a, A> Sync for Atom1<'a, A> {}
