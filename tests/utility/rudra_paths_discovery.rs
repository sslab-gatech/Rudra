struct PathsDiscovery;

impl PathsDiscovery {
    fn discover() {
        unsafe {
            std::ptr::read(12 as *const i32);
            std::ptr::write(12 as *mut i32, 34);
            (12 as *mut i32).write(34);

            std::ptr::slice_from_raw_parts(12 as *const i32, 34);
            std::ptr::slice_from_raw_parts_mut(12 as *mut i32, 34);
            std::slice::from_raw_parts(12 as *const i32, 34);
            std::slice::from_raw_parts_mut(12 as *mut i32, 34);

            std::intrinsics::copy(12 as *const i32, 34 as *mut i32, 56);
            std::intrinsics::copy_nonoverlapping(12 as *const i32, 34 as *mut i32, 56);
            std::ptr::copy(12 as *const i32, 34 as *mut i32, 56);
            std::ptr::copy_nonoverlapping(12 as *const i32, 34 as *mut i32, 56);

            vec![12, 34].set_len(5678);
        }
    }
}
