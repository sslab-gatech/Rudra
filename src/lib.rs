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

mod analyze;
mod call_graph;
mod ext;
pub mod utils;

use rustc::ty::TyCtxt;

use analyze::Analyzer;
use call_graph::CallGraph;
pub use ext::TyCtxtExt;

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
    info!(
        "Found {} functions in the call graph",
        call_graph.num_functions()
    );

    let mut analyzer = Analyzer::new(tcx);

    for local_instance in call_graph.local_safe_fn_iter() {
        let def_path_string = tcx
            .hir()
            .def_path(local_instance.def.def_id())
            .to_string_no_crate();

        // TODO: remove these temporary setups
        if def_path_string == "::buffer[0]::{{impl}}[2]::from[0]"
            || def_path_string == "::trivial[0]"
        {
            info!("Found {:?}", local_instance);
            for &instance in call_graph.reachable_set(local_instance).iter() {
                utils::print_mir(tcx, instance);
            }

            let result = analyzer.analyze(local_instance);
            if result.is_err() {
                // TODO: explain more about the failure
                println!("Analyze failed...");
            }
        }
    }
}
