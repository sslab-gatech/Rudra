/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

pub fn test_copy1<F>(f: F)
where
    F: FnOnce(),
{
    unsafe {
        core::ptr::read(0x1234 as *const u8);
    }
    f();
}

pub fn test_copy2<F, T>(f: F)
where
    F: FnOnce(),
    T: Copy,
{
    unsafe {
        core::ptr::read(0x1234 as *const T);
    }
    f();
}
