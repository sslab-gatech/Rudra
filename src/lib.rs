#![feature(box_patterns)]
#![feature(rustc_private)]

extern crate rustc;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_mir;
extern crate syntax;

#[macro_use]
extern crate log;

mod call_graph;
mod context;
pub mod utils;

use rustc::ty::TyCtxt;

use call_graph::CallGraph;
pub use context::TyCtxtExt;

// Insert rustc arguments at the beginning of the argument list that Crux wants to be
// set per default, for maximal validation power.
pub static CRUX_DEFAULT_ARGS: &[&str] = &[
    "-Zalways-encode-mir",
    "-Zmir-emit-retag",
    "-Zmir-opt-level=0",
    "--cfg=crux",
];

/// Returns the "default sysroot" that Crux will use if no `--sysroot` flag is set.
/// Should be a compile-time constant.
pub fn compile_time_sysroot() -> Option<String> {
    // option_env! is replaced to a constant at compile time
    if option_env!("RUSTC_STAGE").is_some() {
        // This is being built as part of rustc, and gets shipped with rustup.
        // We can rely on the sysroot computation in librustc.
        return None;
    }
    // For builds outside rustc, we need to ensure that we got a sysroot
    // that gets used as a default.  The sysroot computation in librustc would
    // end up somewhere in the build dir.
    // Taken from PR <https://github.com/Manishearth/rust-clippy/pull/911>.
    let home = option_env!("RUSTUP_HOME").or(option_env!("MULTIRUST_HOME"));
    let toolchain = option_env!("RUSTUP_TOOLCHAIN").or(option_env!("MULTIRUST_TOOLCHAIN"));
    Some(match (home, toolchain) {
        (Some(home), Some(toolchain)) => format!("{}/toolchains/{}", home, toolchain),
        _ => option_env!("RUST_SYSROOT")
            .expect("To build Crux without rustup, set the `RUST_SYSROOT` env var at build time")
            .to_owned(),
    })
}

pub fn analyze<'tcx>(tcx: TyCtxt<'tcx>) {
    // collect DefId of all bodies
    let call_graph = CallGraph::new(tcx);
    call_graph.print_mir_availability();
    for local_instance in call_graph.local_safe_fn_iter() {
        trace!("{:?}", local_instance);
        let def_path_string = tcx
            .hir()
            .def_path(local_instance.def.def_id())
            .to_string_no_crate();
        trace!("{}", def_path_string);

        // TODO: remove this temporary setup
        if def_path_string == "::buffer[0]::{{impl}}[2]::from[0]" {
            info!("{:?}", local_instance);
            utils::print_mir(tcx, local_instance);
        }
    }
}
