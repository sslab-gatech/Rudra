/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

struct Atom3<P>(P);

unsafe impl<P> Send for Atom3<P>
where 
    P: Copy + Send
{ }

unsafe impl<P> Sync for Atom3<P>
where
    P: Copy + Clone
{ }

struct Atom4<P>(P);

unsafe impl<P> Sync for Atom4<P>
where
    P: Sync
{}