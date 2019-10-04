#![feature(rustc_private)]

extern crate rustc;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate syntax;

mod hir_visitor;

use rustc::ty::TyCtxt;
use rustc_interface::interface::Compiler;

use hir_visitor::{FunctionCollector, ModuleCollector};

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

pub fn analyze<'tcx>(compiler: &Compiler, tcx: TyCtxt<'tcx>) {
    // collect functions in hir
    let mut function_collector = FunctionCollector::new(&tcx);
    function_collector.collect_functions();

    // collect modules in hir
    let mut module_collector = ModuleCollector::new(&tcx);
    module_collector.collect_modules();

    // print all mods
    for span in module_collector.modules() {
        let source_map = compiler.source_map();
        println!(
            "{} | {}",
            source_map.span_to_string(span.clone()),
            source_map.span_to_snippet(span.clone()).unwrap()
        );
    }

    // print all crates
    let crates = tcx
        .crates()
        .iter()
        .map(|krate| tcx.original_crate_name(krate.clone()))
        .collect::<Vec<_>>();
    println!("Crates: {:?}", crates);
}
