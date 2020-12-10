#![feature(backtrace)]
#![feature(box_patterns)]
#![feature(rustc_private)]
#![feature(try_blocks)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_span;

#[macro_use]
extern crate if_chain;
#[macro_use]
extern crate log as log_crate;

#[macro_use]
mod macros;

pub mod algorithm;
mod analysis;
pub mod context;
pub mod ir;
pub mod log;
pub mod prelude;
pub mod report;
pub mod utils;

use rustc_middle::ty::TyCtxt;

use crate::analysis::solver::SolverW1;
use crate::analysis::{CallGraph, SendSyncChecker, SimpleAnderson, UnsafeDestructor};
use crate::context::RudraCtxtOwner;
use crate::log::Verbosity;

// Insert rustc arguments at the beginning of the argument list that Rudra wants to be
// set per default, for maximal validation power.
pub static RUDRA_DEFAULT_ARGS: &[&str] =
    &["-Zalways-encode-mir", "-Zmir-opt-level=0", "--cfg=rudra"];

#[derive(Debug, Clone, Copy)]
pub struct RudraConfig {
    pub verbosity: Verbosity,
    pub call_graph_enabled: bool,
    pub unsafe_destructor_enabled: bool,
    pub simple_anderson_enabled: bool,
    pub send_sync_enabled: bool,
}

impl Default for RudraConfig {
    fn default() -> Self {
        RudraConfig {
            verbosity: Verbosity::Normal,
            call_graph_enabled: false,
            unsafe_destructor_enabled: true,
            simple_anderson_enabled: false,
            send_sync_enabled: true,
        }
    }
}

/// Returns the "default sysroot" that Rudra will use if no `--sysroot` flag is set.
/// Should be a compile-time constant.
pub fn compile_time_sysroot() -> Option<String> {
    // option_env! is replaced to a constant at compile time
    if option_env!("RUSTC_STAGE").is_some() {
        // This is being built as part of rustc, and gets shipped with rustup.
        // We can rely on the sysroot computation in librustc.
        return None;
    }

    // For builds outside rustc, we need to ensure that we got a sysroot
    // that gets used as a default. The sysroot computation in librustc would
    // end up somewhere in the build dir.
    // Taken from PR <https://github.com/Manishearth/rust-clippy/pull/911>.
    let home = option_env!("RUSTUP_HOME").or(option_env!("MULTIRUST_HOME"));
    let toolchain = option_env!("RUSTUP_TOOLCHAIN").or(option_env!("MULTIRUST_TOOLCHAIN"));
    Some(match (home, toolchain) {
        (Some(home), Some(toolchain)) => format!("{}/toolchains/{}", home, toolchain),
        _ => option_env!("RUST_SYSROOT")
            .expect("To build Rudra without rustup, set the `RUST_SYSROOT` env var at build time")
            .to_owned(),
    })
}

fn run_analysis<F, R>(name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    progress_info!("{} analysis started", name);
    let result = f();
    progress_info!("{} analysis finished", name);
    result
}

pub fn analyze<'tcx>(tcx: TyCtxt<'tcx>, config: RudraConfig) {
    // workaround to mimic arena lifetime
    let rcx_owner = RudraCtxtOwner::new(tcx);
    let rcx = &*Box::leak(Box::new(rcx_owner));

    // shadow the variable tcx
    #[allow(unused_variables)]
    let tcx = ();

    // Unsafe destructor analysis
    if config.unsafe_destructor_enabled {
        run_analysis("UnsafeDestructor", || {
            let mut unsafe_destructor = UnsafeDestructor::new(rcx);
            unsafe_destructor.analyze();
        })
    }

    // Send/Sync analysis
    if config.send_sync_enabled {
        run_analysis("SendSyncChecker", || {
            let send_sync_checker = SendSyncChecker::new(rcx);
            send_sync_checker.analyze();
        })
    }

    if config.call_graph_enabled {
        // collect DefId of all bodies
        let call_graph = run_analysis("CallGraph", || {
            let call_graph = CallGraph::new(rcx);
            info!(
                "Found {} functions in the call graph",
                call_graph.num_functions()
            );
            call_graph
        });

        // Simple anderson analysis
        if config.simple_anderson_enabled {
            run_analysis("SimpleAnderson", || {
                let mut simple_anderson = SimpleAnderson::new(rcx);

                for local_instance in call_graph.local_safe_fn_iter() {
                    let def_path_string = rcx
                        .tcx()
                        .hir()
                        .def_path(local_instance.def.def_id().expect_local())
                        .to_string_no_crate();

                    // TODO: remove these temporary setups
                    if def_path_string == "::buffer[0]::{{impl}}[2]::from[0]"
                        || def_path_string.starts_with("::rudra_test")
                    {
                        info!("Found {:?}", local_instance);
                        println!("Target {}", def_path_string);
                        simple_anderson.analyze(local_instance);

                        let _solver = SolverW1::solve(&simple_anderson);
                        // TODO: report solver result
                    }
                }
            })
        }

        // TODO: remove these temporary setups
        // Call graph testing
        for local_instance in call_graph.local_safe_fn_iter() {
            let def_path_string = rcx
                .tcx()
                .hir()
                .def_path(local_instance.def.def_id().expect_local())
                .to_string_no_crate();

            if def_path_string == "::buffer[0]::{{impl}}[2]::from[0]"
                || def_path_string.starts_with("::rudra_test")
            {
                info!("Found {:?}", local_instance);
                for &instance in call_graph.reachable_set(local_instance).iter() {
                    utils::print_mir(rcx.tcx(), instance);
                }
            }
        }
    }
}
