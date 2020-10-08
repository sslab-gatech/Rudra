/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

struct Atom1<P>(P);
unsafe impl<P: Clone> Send for Atom1<P> {}
unsafe impl<P: Copy + Sync> Sync for Atom1<P> {}

struct Atom2<P>(P);
unsafe impl<P> Sync for Atom2<P> {}