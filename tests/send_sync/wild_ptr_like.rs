/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["SendSyncChecker"]
```
!*/

use std::convert::AsRef;
use std::sync::Arc;

// `P` needs to be `Send + Sync` for `Atom1<P>` to be `Send`.
pub struct Atom1<P>(Arc<P>);
unsafe impl<P: Send> Send for Atom1<P> {}

impl<P> AsRef<P> for Atom1<P> {
    fn as_ref(&self) -> &P {
        self.0.as_ref()
    }
}

impl<P> Clone for Atom1<P> {
    fn clone(&self) -> Self {
        Atom1(Arc::clone(&self.0))
    }
}