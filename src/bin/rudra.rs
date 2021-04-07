#![feature(backtrace)]
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;

#[macro_use]
extern crate log;

use std::env;

use rustc_driver::Compilation;
use rustc_interface::{interface::Compiler, Queries};

use rudra::log::Verbosity;
use rudra::report::{default_report_logger, init_report_logger};
use rudra::{analyze, compile_time_sysroot, progress_info, RudraConfig, RUDRA_DEFAULT_ARGS};

struct RudraCompilerCalls {
    config: RudraConfig,
}

impl RudraCompilerCalls {
    fn new(config: RudraConfig) -> RudraCompilerCalls {
        RudraCompilerCalls { config }
    }
}

impl rustc_driver::Callbacks for RudraCompilerCalls {
    fn after_analysis<'tcx>(
        &mut self,
        compiler: &Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        compiler.session().abort_if_errors();

        rudra::log::setup_logging(self.config.verbosity).expect("Rudra failed to initialize");

        debug!("Input file name: {}", compiler.input().source_name());
        debug!("Crate name: {}", queries.crate_name().unwrap().peek_mut());

        progress_info!("Rudra started");
        queries.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            analyze(tcx, self.config);
        });
        progress_info!("Rudra finished");

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

    // Some options have different defaults in Rudra than in plain rustc; apply those by making
    // them the first arguments after the binary name (but later arguments can overwrite them).
    args.splice(
        1..1,
        rudra::RUDRA_DEFAULT_ARGS.iter().map(ToString::to_string),
    );

    // Invoke compiler, and handle return code.
    let exit_code = rustc_driver::catch_with_exit_code(move || {
        rustc_driver::run_compiler(&args, callbacks, None, None)
    });

    exit_code
}

fn parse_config() -> (RudraConfig, Vec<String>) {
    // collect arguments
    let mut config = RudraConfig::default();

    let mut rustc_args = vec![];
    for arg in std::env::args() {
        match arg.as_str() {
            "-Zrudra-enable-unsafe-destructor" => {
                config.unsafe_destructor_enabled = true;
            }
            "-Zrudra-disable-unsafe-destructor" => {
                config.unsafe_destructor_enabled = false;
            }
            "-Zrudra-enable-send-sync-variance" => config.send_sync_variance_enabled = true,
            "-Zrudra-disable-send-sync-variance" => config.send_sync_variance_enabled = false,
            "-Zrudra-enable-unsafe-dataflow" => config.unsafe_dataflow_enabled = true,
            "-Zrudra-disable-unsafe-dataflow" => config.unsafe_dataflow_enabled = false,
            "-v" => config.verbosity = Verbosity::Verbose,
            "-vv" => config.verbosity = Verbosity::Trace,
            _ => {
                rustc_args.push(arg);
            }
        }
    }

    (config, rustc_args)
}

fn main() {
    rustc_driver::install_ice_hook(); // ICE: Internal Compilation Error

    let exit_code = {
        // initialize the report logger
        // `logger_handle` must be nested because it flushes the logs when it goes out of the scope
        let (config, mut rustc_args) = parse_config();
        let _logger_handle = init_report_logger(default_report_logger());

        // init rustc logger
        if env::var_os("RUSTC_LOG").is_some() {
            rustc_driver::init_rustc_env_logger();
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
        rustc_args.splice(1..1, RUDRA_DEFAULT_ARGS.iter().map(ToString::to_string));

        debug!("rustc arguments: {:?}", &rustc_args);
        run_compiler(rustc_args, &mut RudraCompilerCalls::new(config))
    };

    std::process::exit(exit_code)
}
