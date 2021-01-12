/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncVariance"]
```
!*/

struct Atom<P, Q>(P, Q);
unsafe impl<P: Send, Q> Sync for Atom<P, Q>
where
    Q: Copy,
    P: Sync,
{
}
