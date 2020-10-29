/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

struct Atom<P>(P);
unsafe impl<P: Ord + Sync> Sync for Atom<P> {}
