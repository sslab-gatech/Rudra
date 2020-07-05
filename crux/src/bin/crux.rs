#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;

#[macro_use]
extern crate log;

use std::env;

use rustc_driver::Compilation;
use rustc_interface::{interface::Compiler, Queries};

use dotenv::dotenv;

use crux::report::{default_report_logger, init_report_logger};
use crux::{analyze, compile_time_sysroot, CruxAnalysisConfig, CRUX_DEFAULT_ARGS};

struct CruxCompilerCalls {
    config: CruxAnalysisConfig,
}

impl CruxCompilerCalls {
    fn new(config: CruxAnalysisConfig) -> CruxCompilerCalls {
        CruxCompilerCalls { config }
    }
}

impl rustc_driver::Callbacks for CruxCompilerCalls {
    fn after_analysis<'tcx>(
        &mut self,
        compiler: &Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        compiler.session().abort_if_errors();

        debug!("Input file name: {}", compiler.input().source_name());
        debug!("Crate name: {}", queries.crate_name().unwrap().peek_mut());

        queries.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            analyze(tcx, self.config);
        });
        compiler.session().abort_if_errors();

        Compilation::Stop
    }
}

/// Execute a compiler with the given CLI arguments and callbacks.
fn run_compiler(
    mut args: Vec<String>,
    callbacks: &mut (dyn rustc_driver::Callbacks + Send),
) -> i32 {
    // Make sure we use the right default sysroot. The default sysroot is wrong,
    // because `get_or_default_sysroot` in `librustc_session` bases that on `current_exe`.
    //
    // Make sure we always call `compile_time_sysroot` as that also does some sanity-checks
    // of the environment we were built in.
    // FIXME: Ideally we'd turn a bad build env into a compile-time error via CTFE or so.
    if let Some(sysroot) = compile_time_sysroot() {
        let sysroot_flag = "--sysroot";
        if !args.iter().any(|e| e == sysroot_flag) {
            // We need to overwrite the default that librustc_session would compute.
            args.push(sysroot_flag.to_owned());
            args.push(sysroot);
        }
    }

    // Some options have different defaults in Crux than in plain rustc; apply those by making
    // them the first arguments after the binary name (but later arguments can overwrite them).
    args.splice(
        1..1,
        crux::CRUX_DEFAULT_ARGS.iter().map(ToString::to_string),
    );

    // Invoke compiler, and handle return code.
    let exit_code = rustc_driver::catch_with_exit_code(move || {
        rustc_driver::run_compiler(&args, callbacks, None, None)
    });

    exit_code
}

fn main() {
    rustc_driver::install_ice_hook(); // ICE: Internal Compilation Error

    let exit_code = if env::var_os("CRUX_BE_RUSTC").is_some() {
        // If the environment asks us to actually be rustc, then do that.
        rustc_driver::init_rustc_env_logger();

        // We cannot use `rustc_driver::main` as we need to adjust the CLI arguments.
        let mut callbacks = rustc_driver::TimePassesCallbacks::default();
        run_compiler(env::args().collect(), &mut callbacks)
    } else {
        // Otherwise, run Crux analysis

        // init Crux logger
        dotenv().ok();
        let env = env_logger::Env::new()
            .filter("CRUX_LOG")
            .write_style("CRUX_LOG_STYLE");
        env_logger::init_from_env(env);

        // init rustc logger
        if env::var_os("RUSTC_LOG").is_some() {
            rustc_driver::init_rustc_env_logger();
        }

        // init report logger
        let _logger_handle = init_report_logger(default_report_logger());

        // collect arguments
        let mut config = CruxAnalysisConfig::default();

        let mut crate_name = None;
        let mut rustc_args = vec![];

        for arg in std::env::args() {
            if rustc_args.is_empty() {
                // Very first arg: crate name
                crate_name = Some(arg.clone());
                rustc_args.push(arg);
            } else {
                match arg.as_str() {
                    "-Zcrux-enable-simple-anderson" => {
                        config.simple_anderson_enabled = true;
                    }
                    "-Zcrux-disable-simple-anderson" => {
                        config.simple_anderson_enabled = false;
                    }
                    "-Zcrux-enable-unsafe-destructor" => {
                        config.unsafe_destructor_enabled = true;
                    }
                    "-Zcrux-disable-unsafe-destructor" => {
                        config.unsafe_destructor_enabled = false;
                    }
                    _ => {
                        rustc_args.push(arg);
                    }
                }
            }
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

        debug!("rustc arguments: {:?}", &rustc_args);

        run_compiler(rustc_args, &mut CruxCompilerCalls::new(config))
    };

    std::process::exit(exit_code)
}
