/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

#![allow(dead_code)]
// This is valid for channel-like types that only transfers the ownership.
// This is invalid if the outer type implements dereference or peek functionality.
// We emit error by default for now.
struct Container<P, Q>(P, Q);
unsafe impl<P: Sync, Q: Send> Sync for Container<P, Q> {}

impl<P, Q> Container<P, Q> {
    fn append_to_queue(&self, _msg: Q) {}

    fn peek_queue_end(&self) -> Result<&Q, ()> {
        Ok(&self.1)
    }
}
