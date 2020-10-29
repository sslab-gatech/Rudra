/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

struct Atom<P>(P);
unsafe impl<P: Ord> Send for Atom<P> {}
