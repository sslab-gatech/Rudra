/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

use std::marker::PhantomData;

struct Atom1<'a, P, Q, R> {
    _marker0: PhantomData<P>,
    _marker1: PhantomData<Option<*mut P>>,
    _marker2: PhantomData<Box<(&'a mut Q, Box<Result<R, i32>>)>>,
}
unsafe impl<'a, A: Send, B, C> Send for Atom1<'a, A, B, C> {}
unsafe impl<'a, A: Sync, B, C> Sync for Atom1<'a, A, B, C> {}
