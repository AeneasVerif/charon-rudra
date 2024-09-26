use crate::context::Ctx;
use charon_lib::ast::names::Name;
use charon_lib::name_matcher::Pattern;
use maplit::hashmap;
use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::rudra::analysis::UnsafeDataflowBehaviorFlag;

/*
How to find a path for unknown item:
1. Modify tests/utility/rurda_paths_discovery.rs
2. cargo run --bin rudra -- --crate-type lib tests/utility/rudra_paths_discovery.rs

For temporary debugging, you can also change this line in `prelude.rs`
`let names = self.get_def_path(def_id);`
to
`let names = dbg!(self.get_def_path(def_id));`
*/
// Strong bypasses
pub const PTR_READ: [&str; 3] = ["core", "ptr", "read"];
pub const PTR_DIRECT_READ: [&str; 5] = ["core", "ptr", "const_ptr", "<_>", "read"];
//pub const PTR_DIRECT_READ: [&str; 5] = ["core", "ptr", "const_ptr", "<impl *const T>", "read"];

pub const INTRINSICS_COPY: [&str; 3] = ["core", "intrinsics", "copy"];
pub const INTRINSICS_COPY_NONOVERLAPPING: [&str; 3] = ["core", "intrinsics", "copy_nonoverlapping"];

pub const VEC_SET_LEN: [&str; 4] = ["alloc", "vec", "<Vec<_>>", "set_len"];
pub const VEC_FROM_RAW_PARTS: [&str; 4] = ["alloc", "vec", "<Vec<_>>", "from_raw_parts"];

// Weak bypasses
pub const TRANSMUTE: [&str; 4] = ["core", "intrinsics", "", "transmute"];

pub const PTR_WRITE: [&str; 3] = ["core", "ptr", "write"];
//pub const PTR_DIRECT_WRITE: [&str; 5] = ["core", "ptr", "mut_ptr", "<impl *mut T>", "write"];
pub const PTR_DIRECT_WRITE: [&str; 5] = ["core", "ptr", "mut_ptr", "<_>", "write"];

//pub const PTR_AS_REF: [&str; 5] = ["core", "ptr", "const_ptr", "<impl *const T>", "as_ref"];
//pub const PTR_AS_MUT: [&str; 5] = ["core", "ptr", "mut_ptr", "<impl *mut T>", "as_mut"];
pub const PTR_AS_REF: [&str; 5] = ["core", "ptr", "const_ptr", "<_>", "as_ref"];
pub const PTR_AS_MUT: [&str; 5] = ["core", "ptr", "mut_ptr", "<_>", "as_mut"];
pub const NON_NULL_AS_REF: [&str; 5] = ["core", "ptr", "non_nul", "<NonNull<T>>", "as_ref"];
pub const NON_NULL_AS_MUT: [&str; 5] = ["core", "ptr", "non_nul", "<NonNull<T>>", "as_mut"];

pub const SLICE_GET_UNCHECKED: [&str; 4] = ["core", "slice", "<[T]>", "get_unchecked"];
pub const SLICE_GET_UNCHECKED_MUT: [&str; 4] = ["core", "slice", "<[T]>", "get_unchecked_mut"];

pub const PTR_SLICE_FROM_RAW_PARTS: [&str; 3] = ["core", "ptr", "slice_from_raw_parts"];
pub const PTR_SLICE_FROM_RAW_PARTS_MUT: [&str; 3] = ["core", "ptr", "slice_from_raw_parts_mut"];
pub const SLICE_FROM_RAW_PARTS: [&str; 3] = ["core", "slice", "from_raw_parts"];
pub const SLICE_FROM_RAW_PARTS_MUT: [&str; 3] = ["core", "slice", "from_raw_parts_mut"];

// Generic function call
pub const PTR_DROP_IN_PLACE: [&str; 3] = ["core", "ptr", "drop_in_place"];
pub const PTR_DIRECT_DROP_IN_PLACE: [&str; 5] = ["core", "ptr", "mut_ptr", "<_>", "drop_in_place"];
//    ["core", "ptr", "mut_ptr", "<impl *mut T>", "drop_in_place"];

pub struct PathSet {
    set: Vec<(String, Pattern)>,
}

fn slice_to_string(a: &[&str]) -> String {
    a.iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

impl PathSet {
    pub fn new(path_arr: &[&[&str]]) -> Self {
        let mut set = Vec::new();
        for path in path_arr {
            let name = slice_to_string(path);
            let pat = Pattern::parse(&name).unwrap();
            set.push((name, pat));
        }

        PathSet { set }
    }

    pub fn contains(&self, ctx: &Ctx, target: &Name) -> bool {
        self.set
            .iter()
            .find(|p| p.1.matches(&ctx.crate_data, target))
            .is_some()
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

pub static GENERIC_FN_LIST: Lazy<PathSet> =
    Lazy::new(move || PathSet::new(&[&PTR_DROP_IN_PLACE, &PTR_DIRECT_DROP_IN_PLACE]));

type PathMap = HashMap<String, UnsafeDataflowBehaviorFlag>;

pub static STRONG_BYPASS_MAP: Lazy<PathMap> = Lazy::new(move || {
    use UnsafeDataflowBehaviorFlag as BehaviorFlag;

    hashmap! {
        slice_to_string(&PTR_READ)=> BehaviorFlag::READ_FLOW,
        slice_to_string(&PTR_DIRECT_READ)=> BehaviorFlag::READ_FLOW,
        //
        slice_to_string(&INTRINSICS_COPY)=> BehaviorFlag::COPY_FLOW,
        slice_to_string(&INTRINSICS_COPY_NONOVERLAPPING)=> BehaviorFlag::COPY_FLOW,
        //
        slice_to_string(&VEC_SET_LEN)=> BehaviorFlag::VEC_SET_LEN,
        //
        slice_to_string(&VEC_FROM_RAW_PARTS)=> BehaviorFlag::VEC_FROM_RAW,
    }
});

pub static WEAK_BYPASS_MAP: Lazy<PathMap> = Lazy::new(move || {
    use UnsafeDataflowBehaviorFlag as BehaviorFlag;

    hashmap! {
        slice_to_string(&TRANSMUTE)=> BehaviorFlag::TRANSMUTE,
        //
        slice_to_string(&PTR_WRITE)=> BehaviorFlag::WRITE_FLOW,
        slice_to_string(&PTR_DIRECT_WRITE)=> BehaviorFlag::WRITE_FLOW,
        //
        slice_to_string(&PTR_AS_REF)=> BehaviorFlag::PTR_AS_REF,
        slice_to_string(&PTR_AS_MUT)=> BehaviorFlag::PTR_AS_REF,
        slice_to_string(&NON_NULL_AS_REF)=> BehaviorFlag::PTR_AS_REF,
        slice_to_string(&NON_NULL_AS_MUT)=> BehaviorFlag::PTR_AS_REF,
        //
        slice_to_string(&SLICE_GET_UNCHECKED)=> BehaviorFlag::SLICE_UNCHECKED,
        slice_to_string(&SLICE_GET_UNCHECKED_MUT)=> BehaviorFlag::SLICE_UNCHECKED,
        //
        slice_to_string(&PTR_SLICE_FROM_RAW_PARTS)=> BehaviorFlag::SLICE_FROM_RAW,
        slice_to_string(&PTR_SLICE_FROM_RAW_PARTS_MUT)=> BehaviorFlag::SLICE_FROM_RAW,
        slice_to_string(&SLICE_FROM_RAW_PARTS)=> BehaviorFlag::SLICE_FROM_RAW,
        slice_to_string(&SLICE_FROM_RAW_PARTS_MUT)=> BehaviorFlag::SLICE_FROM_RAW,
    }
});
