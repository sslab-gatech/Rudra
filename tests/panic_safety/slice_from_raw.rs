/*!
```rudra-test
test_type = "normal"
expected_analyzers = ["UnsafeDataflow"]
```
!*/

pub trait Writer {
    fn write_bytes(&mut self, slice: &[u8]) -> Result<(), ()>;
    fn write_u8(&mut self, value: u8) -> Result<(), ()> {
        let slice = unsafe { std::slice::from_raw_parts(&value, 1) };
        self.write_bytes(slice)
    }
}