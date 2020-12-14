/*
How to find path for unknown item
1. Modify tests/utility/rurda_paths_discovery.rs
2. cargo run --bin rudra -- --crate-type lib tests/utility/rudra_paths_discovery.rs
*/
pub const PTR_READ: [&str; 3] = ["core", "ptr", "read"];
pub const PTR_WRITE: [&str; 3] = ["core", "ptr", "write"];
pub const PTR_SLICE_FROM_RAW_PARTS: [&str; 3] = ["core", "ptr", "slice_from_raw_parts"];
pub const PTR_SLICE_FROM_RAW_PARTS_MUT: [&str; 3] = ["core", "ptr", "slice_from_raw_parts_mut"];
pub const SLICE_FROM_RAW_PARTS: [&str; 3] = ["core", "slice", "from_raw_parts"];
pub const SLICE_FROM_RAW_PARTS_MUT: [&str; 3] = ["core", "slice", "from_raw_parts_mut"];
pub const INTRINSICS_COPY: [&str; 3] = ["core", "intrinsics", "copy"];
pub const INTRINSICS_COPY_NONOVERLAPPING: [&str; 3] = ["core", "intrinsics", "copy_nonoverlapping"];
