// cargo run --bin rudra -- --crate-type lib tests/utility/rudra_paths_discovery.rs
use std::ptr::NonNull;

struct PathsDiscovery;

impl PathsDiscovery {
    fn discover() {
        unsafe {
            // Strong bypasses
            std::mem::transmute::<_, *mut i32>(12 as *const i32);

            std::ptr::read(12 as *const i32);
            (12 as *const i32).read();

            std::intrinsics::copy(12 as *const i32, 34 as *mut i32, 56);
            std::intrinsics::copy_nonoverlapping(12 as *const i32, 34 as *mut i32, 56);
            std::ptr::copy(12 as *const i32, 34 as *mut i32, 56);
            std::ptr::copy_nonoverlapping(12 as *const i32, 34 as *mut i32, 56);

            vec![12, 34].set_len(5678);
            std::vec::Vec::from_raw_parts(12 as *mut i32, 34, 56);

            // Weak bypasses
            (12 as *mut i32).write(34);
            std::ptr::write(12 as *mut i32, 34);

            (12 as *const i32).as_ref();
            (12 as *mut i32).as_mut();

            let mut ptr = NonNull::new(1234 as *mut i32).unwrap();
            ptr.as_ref();
            ptr.as_mut();

            [12, 34].get_unchecked(0);
            [12, 34].get_unchecked_mut(0);

            std::ptr::slice_from_raw_parts(12 as *const i32, 34);
            std::ptr::slice_from_raw_parts_mut(12 as *mut i32, 34);
            std::slice::from_raw_parts(12 as *const i32, 34);
            std::slice::from_raw_parts_mut(12 as *mut i32, 34);
        }
    }
}
