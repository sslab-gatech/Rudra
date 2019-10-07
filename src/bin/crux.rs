#![feature(rustc_private)]

extern crate rustc;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate syntax;

use rustc_driver::Compilation;
use rustc_interface::interface::Compiler;

use crux::{analyze, compile_time_sysroot, CRUX_DEFAULT_ARGS};

struct CruxCompilerCalls {}

impl CruxCompilerCalls {
    fn new() -> CruxCompilerCalls {
        CruxCompilerCalls {}
    }
}

impl rustc_driver::Callbacks for CruxCompilerCalls {
    fn after_analysis(&mut self, compiler: &Compiler) -> Compilation {
        compiler.session().abort_if_errors();

        println!("Input file name: {}", compiler.input().source_name());
        println!("Crate name: {}", compiler.crate_name().unwrap().peek_mut());

        compiler.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            analyze(tcx);
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

    rustc_driver::install_ice_hook(); // ICE : Internal Compilation Error
    let result = rustc_driver::catch_fatal_errors(move || {
        rustc_driver::run_compiler(&rustc_args, &mut CruxCompilerCalls::new(), None, None)
    })
    .and_then(|result| result);
    std::process::exit(result.is_err() as i32);
}
