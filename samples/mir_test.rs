extern "C" {
    fn strcpy(ptr: *const i8);
}

mod inner {
    pub static MSG: &str = "YES";
}

fn main() {
    println!("Hello, World!");
}

unsafe fn other(ptr: *mut u32) {
    *ptr = 0xcafebabe;
}

fn update_ref(ptr: &mut u32) {
    fn nested() -> i32 {
        unsafe { std::ptr::read(0xaaaaaaaa as *const i32) }
    }
    *ptr = 0x12345678;
}

pub fn not_uaf(server_name: &str) {
    use std::ffi::CString;

    unsafe {
        strcpy(CString::new(server_name).unwrap().as_ptr());
    }
}

trait MyTrait {
    fn required(&self);

    fn provided(&self) {
        println!("This is a provided function");
    }
}

struct MyStruct {}

impl MyStruct {
    fn new() -> Self {
        MyStruct {}
    }
}

impl MyTrait for MyStruct {
    fn required(&self) {
        println!("This is a required function");
    }
}
