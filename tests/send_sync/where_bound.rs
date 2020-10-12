/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

struct Atom3<P, Q>(P, Q);

unsafe impl<P, Q> Send for Atom3<P, Q> where Q: Send, P: Copy + Send {}

unsafe impl<P: Send, Q> Sync for Atom3<P, Q> where Q: Copy, P: Sync {}

struct Atom4<P>(P);

unsafe impl<P> Sync for Atom4<P> where P: Sync {}

// If an object is `Sync`, it is also `Send`
struct Atom5<P>(P);

unsafe impl<P> Send for Atom5<P> where P: Sync {} 