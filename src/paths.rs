use std::collections::HashSet;

use rustc_span::Symbol;

use once_cell::sync::Lazy;

/*
How to find a path for unknown item:
1. Modify tests/utility/rurda_paths_discovery.rs
2. cargo run --bin rudra -- --crate-type lib tests/utility/rudra_paths_discovery.rs

For temporary debugging, you can also change this line in `prelude.rs`
`let names = self.get_def_path(def_id);`
to
`let names = dbg!(self.get_def_path(def_id));`
*/
pub const PTR_READ: [&str; 3] = ["core", "ptr", "read"];
pub const PTR_DIRECT_READ: [&str; 5] = ["core", "ptr", "const_ptr", "<impl *const T>", "read"];

pub const INTRINSICS_COPY: [&str; 3] = ["core", "intrinsics", "copy"];
pub const INTRINSICS_COPY_NONOVERLAPPING: [&str; 3] = ["core", "intrinsics", "copy_nonoverlapping"];

pub const VEC_SET_LEN: [&str; 4] = ["alloc", "vec", "Vec", "set_len"];
pub const VEC_FROM_RAW_PARTS: [&str; 4] = ["alloc", "vec", "Vec", "from_raw_parts"];

pub const PTR_DROP_IN_PLACE: [&str; 3] = ["core", "ptr", "drop_in_place"];
pub const PTR_DIRECT_DROP_IN_PLACE: [&str; 5] =
    ["core", "ptr", "mut_ptr", "<impl *mut T>", "drop_in_place"];

pub const TRANSMUTE: [&str; 4] = ["core", "intrinsics", "", "transmute"];

pub const PTR_WRITE: [&str; 3] = ["core", "ptr", "write"];
pub const PTR_DIRECT_WRITE: [&str; 5] = ["core", "ptr", "mut_ptr", "<impl *mut T>", "write"];

pub const PTR_AS_REF: [&str; 5] = ["core", "ptr", "const_ptr", "<impl *const T>", "as_ref"];
pub const PTR_AS_MUT: [&str; 5] = ["core", "ptr", "mut_ptr", "<impl *mut T>", "as_mut"];
pub const NON_NULL_AS_REF: [&str; 5] = ["core", "ptr", "non_nul", "NonNull", "as_ref"];
pub const NON_NULL_AS_MUT: [&str; 5] = ["core", "ptr", "non_nul", "NonNull", "as_mut"];

pub const SLICE_GET_UNCHECKED: [&str; 4] = ["core", "slice", "<impl [T]>", "get_unchecked"];
pub const SLICE_GET_UNCHECKED_MUT: [&str; 4] = ["core", "slice", "<impl [T]>", "get_unchecked_mut"];

pub const PTR_SLICE_FROM_RAW_PARTS: [&str; 3] = ["core", "ptr", "slice_from_raw_parts"];
pub const PTR_SLICE_FROM_RAW_PARTS_MUT: [&str; 3] = ["core", "ptr", "slice_from_raw_parts_mut"];
pub const SLICE_FROM_RAW_PARTS: [&str; 3] = ["core", "slice", "from_raw_parts"];
pub const SLICE_FROM_RAW_PARTS_MUT: [&str; 3] = ["core", "slice", "from_raw_parts_mut"];

pub struct PathSet {
    set: HashSet<Vec<Symbol>>,
}

impl PathSet {
    pub fn new(path_arr: &[&[&str]]) -> Self {
        let mut set = HashSet::new();
        for path in path_arr {
            let path_vec = path.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>();
            set.insert(path_vec);
        }

        PathSet { set }
    }

    pub fn contains(&self, target: &Vec<Symbol>) -> bool {
        self.set.contains(target)
    }
}

/// Special path used only for path discovery
pub static SPECIAL_PATH_DISCOVERY: Lazy<PathSet> =
    Lazy::new(move || PathSet::new(&[&["rudra_paths_discovery", "PathsDiscovery", "discover"]]));

pub static STRONG_LIFETIME_BYPASS_LIST: Lazy<PathSet> = Lazy::new(move || {
    PathSet::new(&[
        &PTR_READ,
        &PTR_DIRECT_READ,
        //
        &INTRINSICS_COPY,
        &INTRINSICS_COPY_NONOVERLAPPING,
        //
        &VEC_SET_LEN,
        &VEC_FROM_RAW_PARTS,
        //
        &PTR_DROP_IN_PLACE,
        &PTR_DIRECT_DROP_IN_PLACE,
    ])
});

pub static WEAK_LIFETIME_BYPASS_LIST: Lazy<PathSet> = Lazy::new(move || {
    PathSet::new(&[
        &TRANSMUTE,
        //
        &PTR_WRITE,
        &PTR_DIRECT_WRITE,
        //
        &PTR_AS_REF,
        &PTR_AS_MUT,
        &NON_NULL_AS_REF,
        &NON_NULL_AS_MUT,
        //
        &SLICE_GET_UNCHECKED,
        &SLICE_GET_UNCHECKED_MUT,
        //
        &PTR_SLICE_FROM_RAW_PARTS,
        &PTR_SLICE_FROM_RAW_PARTS_MUT,
        &SLICE_FROM_RAW_PARTS,
        &SLICE_FROM_RAW_PARTS_MUT,
    ])
});
