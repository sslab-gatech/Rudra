/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

use std::marker::PhantomData;

struct Atom1<P, Q> {
    _marker: PhantomData<Q>,
    x: P,
}
unsafe impl<A: Send, B> Send for Atom1<A, B> {}
unsafe impl<A: Sync, B> Sync for Atom1<A, B> {}
