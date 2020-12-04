/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

struct Atom1<P, Q>(P, Q);
unsafe impl<P: Copy, Q> Send for Atom1<P, Q> where Q: Copy {}

// TODO: Need a more complicated test case motivated by Beef?
