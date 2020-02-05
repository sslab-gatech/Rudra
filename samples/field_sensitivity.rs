// cargo run -- --crate-type lib samples/field_sensitivity.rs 2>stderr

#[derive(Debug)]
struct MyStruct {
    x: i32,
    y: i32,
}

pub fn crux_test_struct() {
    // dbg!(MyStruct { x: 1, y: 2 });

    let p: *const i32;
    {
        let composite = MyStruct { x: 1, y: 2 };
        p = &composite.x;
        dbg!(composite);
    } // struct instance `composite` drops here

    dbg!(unsafe { *p }); // Reading a dead location
}
