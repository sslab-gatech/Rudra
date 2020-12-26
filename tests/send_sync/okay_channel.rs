/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

#![allow(dead_code)]
// This is valid for channel-like types that only transfers the ownership.
// This is invalid if the outer type implements dereference or peek functionality.
// We emit error by default for now.
struct Channel<P, Q>(P, Q);
unsafe impl<P: Sync, Q: Send> Sync for Channel<P, Q> {}

impl<P, Q> Channel<P, Q> {
    fn send(&self, _msg: Box<Q>) {
        
    }
}