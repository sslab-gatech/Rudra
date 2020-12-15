/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

trait Foo: Sync {}

// `Foo` is `Sync`, so this is okay.
struct Atom0<P>(P);
unsafe impl<P: Eq + Foo> Sync for Atom0<P> {}

// `Foo` is `Sync`, which means `Foo` is also `Send`. This is also okay.
struct Atom1<P>(P);
unsafe impl<P: Eq> Send for Atom1<P> where P: Foo {}
