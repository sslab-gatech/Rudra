/*!
```rudra-test
test_type = "normal"
expected_analyzers = []
```
!*/

extern "C" {
    fn extern_unsafe(x: u8);
}

pub struct MyStruct(u8);

// calling extern unsafe function should not alarm
impl Drop for MyStruct {
    fn drop(&mut self) {
        unsafe {
            extern_unsafe(self.0);
        }
    }
}
