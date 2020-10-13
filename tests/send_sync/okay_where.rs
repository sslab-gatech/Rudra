/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

struct Atom1<P, Q>(P, Q);
unsafe impl<P, Q> Send for Atom1<P, Q>
where
    Q: Send,
    P: Copy + Send,
{
}

struct Atom2<P>(P);
unsafe impl<P> Sync for Atom2<P> where P: Sync {}
