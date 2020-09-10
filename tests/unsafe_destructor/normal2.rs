/*!
```crux-test
test_type = "normal"
expected_analyzers = ["UnsafeDestructor"]
```
!*/

// RUSTSEC-2020-0032 simplified
use std::os::raw::c_char;

pub struct StrcCtx {
    pub ptr: *mut c_char,
}

impl Drop for StrcCtx {
    fn drop(&mut self) {
        unsafe {
            let _ = std::ffi::CString::from_raw(self.ptr as *mut std::os::raw::c_char);
        }
    }
}
