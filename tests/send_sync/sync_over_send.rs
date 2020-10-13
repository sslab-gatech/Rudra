/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

// This is valid for channel-like types that only transfers the ownership.
// This is invalid if the outer type implements dereference or peek functionality.
// We emit error by default for now.
struct Container<P, Q>(P, Q);
unsafe impl<P: Sync, Q: Send> Sync for Container<P, Q> {}
