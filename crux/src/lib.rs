#![feature(box_patterns)]
#![feature(rustc_private)]

extern crate rustc;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_mir;
extern crate rustc_span;

#[macro_use]
extern crate log;

mod analyze;
mod call_graph;
pub mod context;
/// Data Structures
pub mod ds;
pub mod error;
pub mod ext;
pub mod ir;
pub mod prelude;
pub mod utils;

use rustc::ty::TyCtxt;

use crate::analyze::solver::SolverW1;
use crate::analyze::SimpleAnderson;
use crate::call_graph::CallGraph;
use crate::context::CruxCtxtOwner;
use crate::error::Error;

// Insert rustc arguments at the beginning of the argument list that Crux wants to be
// set per default, for maximal validation power.
pub static CRUX_DEFAULT_ARGS: &[&str] = &["-Zalways-encode-mir", "-Zmir-opt-level=0", "--cfg=crux"];

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
    let ccx_owner = CruxCtxtOwner::new(tcx);
    let ccx = &ccx_owner;

    // shadow the variable tcx
    #[allow(unused_variables)]
    let tcx = ();

    // collect DefId of all bodies
    let call_graph = CallGraph::new(ccx);
    info!(
        "Found {} functions in the call graph",
        call_graph.num_functions()
    );

    let mut simple_anderson = SimpleAnderson::new(ccx);

    for local_instance in call_graph.local_safe_fn_iter() {
        let def_path_string = ccx
            .tcx()
            .hir()
            .def_path(local_instance.def.def_id().expect_local())
            .to_string_no_crate();

        // TODO: remove these temporary setups
        if def_path_string == "::buffer[0]::{{impl}}[2]::from[0]"
            || def_path_string.starts_with("::crux_test")
        {
            info!("Found {:?}", local_instance);
            for &instance in call_graph.reachable_set(local_instance).iter() {
                utils::print_mir(ccx.tcx(), instance);
            }

            let result = simple_anderson.analyze(local_instance);

            println!("Target {}", def_path_string);
            match result {
                Err(e @ Error::AnalysisUnimplemented(_)) => {
                    println!("Analysis Unimplemented: {:?}", e);
                }
                Err(e @ Error::TranslationUnimplemented(_)) => {
                    println!("Translation Unimplemented: {:?}", e);
                }
                Err(e) => {
                    println!("Analysis failed with error: {:?}", e);
                }
                Ok(_) => {
                    let _solver = SolverW1::solve(&simple_anderson);
                    println!("No error found");
                }
            }
        }
    }
}
