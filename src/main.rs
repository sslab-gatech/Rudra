#![feature(rustc_private)]

extern crate rustc;
extern crate rustc_driver;
extern crate rustc_interface;

mod syntax;

use rustc_driver::Compilation;
use rustc_interface::interface;

use syntax::SyntaxVisitor;

// Insert rustc arguments at the beginning of the argument list that Crux wants to be
// set per default, for maximal validation power.
static CRUX_DEFAULT_ARGS: &[&str] = &[
    "-Zalways-encode-mir",
    "-Zmir-emit-retag",
    "-Zmir-opt-level=0",
    "--cfg=crux",
];

/// Returns the "default sysroot" that Crux will use if no `--sysroot` flag is set.
/// Should be a compile-time constant.
fn compile_time_sysroot() -> Option<String> {
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

struct CruxCompilerCalls {}

impl CruxCompilerCalls {
    fn new() -> CruxCompilerCalls {
        CruxCompilerCalls {}
    }
}

impl rustc_driver::Callbacks for CruxCompilerCalls {
    fn after_analysis(&mut self, compiler: &interface::Compiler) -> Compilation {
        compiler.session().abort_if_errors();

        println!("Input file name: {}", compiler.input().source_name());
        println!("Crate name: {}", compiler.crate_name().unwrap().peek_mut());

        compiler.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            let mut visitor = SyntaxVisitor::new(tcx);
            visitor.collect_functions();
            dbg!(visitor.vec());
        });
        compiler.session().abort_if_errors();

        Compilation::Stop
    }
}

fn main() {
    let mut rustc_args = Vec::new();

    for arg in std::env::args() {
        rustc_args.push(arg);
    }

    if let Some(sysroot) = compile_time_sysroot() {
        let sysroot_flag = "--sysroot";
        if !rustc_args.iter().any(|e| e == sysroot_flag) {
            // We need to overwrite the default that librustc would compute.
            rustc_args.push(sysroot_flag.to_owned());
            rustc_args.push(sysroot);
        }
    }

    // Finally, add the default flags all the way in the beginning, but after the binary name.
    rustc_args.splice(1..1, CRUX_DEFAULT_ARGS.iter().map(ToString::to_string));

    dbg!(&rustc_args);

    rustc_driver::install_ice_hook();
    let result = rustc_driver::catch_fatal_errors(move || {
        rustc_driver::run_compiler(&rustc_args, &mut CruxCompilerCalls::new(), None, None)
    })
    .and_then(|result| result);
    std::process::exit(result.is_err() as i32);
}
