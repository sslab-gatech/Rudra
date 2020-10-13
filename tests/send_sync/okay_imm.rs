/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

struct Atom<P>(P);
unsafe impl<P: Copy + Sync> Sync for Atom<P> {}
