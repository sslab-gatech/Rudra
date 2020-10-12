/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

struct Atom1<P>(P);
unsafe impl<P: Clone> Send for Atom1<P> {}
unsafe impl<P: Copy + Sync> Sync for Atom1<P> {}

struct Atom2<P, Q>(P, Q);
unsafe impl<P: Sync, Q: Send> Sync for Atom2<P, Q> {}

// For now, we don't catch cases like below
// where `Sync` or `Send` is implemented for a struct without generic parameters
struct Atom3(usize);
unsafe impl Send for Atom3 {}

// If an object is `Sync`, it is also `Send`.
struct Atom4<P>(P);
unsafe impl<P: Sync> Send for Atom4<P> {}