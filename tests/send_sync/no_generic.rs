/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

// `Sync` or `Send` is implemented for a struct without generic parameters
// In most of the case, this is fine
struct Atom(usize);

unsafe impl Sync for Atom {}
unsafe impl Send for Atom {}
