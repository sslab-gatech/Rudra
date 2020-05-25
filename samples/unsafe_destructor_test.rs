// cargo run -- --crate-type lib samples/unsafe_destructor_test.rs 2>stderr

pub struct FooNoDrop<'a> {
    vec: &'a mut Vec<u32>,
}

pub struct FooSafe<'a> {
    vec: &'a mut Vec<u32>,
}

impl Drop for FooSafe<'_> {
    fn drop(&mut self) {
        println!("{}", self.vec.len());
    }
}

pub struct FooUnsafe<'a> {
    vec: &'a mut Vec<u32>,
}

impl Drop for FooUnsafe<'_> {
    fn drop(&mut self) {
        unsafe {
            self.vec.set_len(0);
        }
    }
}

pub struct BarNoDrop {
    vec: Vec<u32>,
}

pub struct BarSafe {
    vec: Vec<u32>,
}

impl Drop for BarSafe {
    fn drop(&mut self) {
        println!("{}", self.vec.len());
    }
}

pub struct BarUnsafe {
    vec: Vec<u32>,
}

impl Drop for BarUnsafe {
    fn drop(&mut self) {
        unsafe {
            self.vec.set_len(0);
        }
    }
}

pub struct BazNoDrop {
    field: u32,
}

pub struct BazSafe {
    field: u32,
}

impl Drop for BazSafe {
    fn drop(&mut self) {
        println!("{}", self.field);
    }
}

pub struct BazUnsafe {
    field: u32,
}

impl Drop for BazUnsafe {
    fn drop(&mut self) {
        let ptr = self.field as usize as *const u8;
        unsafe {
            println!("{}", *ptr);
        }
    }
}
