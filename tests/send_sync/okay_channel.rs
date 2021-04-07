/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

#![allow(dead_code)]
// This is valid for channel-like types that only transfers the ownership.
// This is invalid if the outer type implements dereference or peek functionality.
struct Channel<P, Q>(P, Q);
unsafe impl<P: Send, Q: Send> Sync for Channel<P, Q> {}

impl<P, Q> Channel<P, Q> {
    fn send_p<M>(&self, _msg: M)
    where
        M: Into<P>,
    {
    }
    fn send_q(&self, _msg: Box<Q>) {}
}
