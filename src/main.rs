#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;

use rustc_driver::Compilation;
use rustc_interface::interface;

/// Returns the "default sysroot" that Crux will use if no `--sysroot` flag is set.
/// Should be a compile-time constant.
fn compile_time_sysroot() -> Option<String> {
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
    fn after_parsing(&mut self, _compiler: &interface::Compiler) -> Compilation {
        println!("after parsing");
        Compilation::Continue
    }

    fn after_analysis(&mut self, _compiler: &interface::Compiler) -> Compilation {
        println!("after analysis");
        Compilation::Continue
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

    dbg!(&rustc_args);

    rustc_driver::install_ice_hook();
    let result = rustc_driver::catch_fatal_errors(move || {
        rustc_driver::run_compiler(&rustc_args, &mut CruxCompilerCalls::new(), None, None)
    })
    .and_then(|result| result);
    std::process::exit(result.is_err() as i32);
}
