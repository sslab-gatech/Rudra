/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

// impl `Send` for `PtrLike<Sync>` is okay
// Note that we don't check pointer-likeness yet

struct Atom1<P>(P);
unsafe impl<P: Sync> Send for Atom1<P> {}

struct Atom2<P>(P);
unsafe impl<P> Send for Atom2<P> where P: Sync {}
