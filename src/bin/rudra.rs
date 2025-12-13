#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

#[macro_use]
extern crate log;

use std::env;

use rustc_driver::Compilation;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;
use rustc_session::EarlyDiagCtxt;

use rudra::log::Verbosity;
use rudra::report::{default_report_logger, init_report_logger, ReportLevel};
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
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        compiler.sess.dcx().abort_if_errors();

        rudra::log::setup_logging(self.config.verbosity).expect("Rudra failed to initialize");

        debug!("Crate name: {}", tcx.crate_name(LOCAL_CRATE));

        progress_info!("Rudra started");
        analyze(tcx, self.config);
        progress_info!("Rudra finished");

        compiler.sess.dcx().abort_if_errors();
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
        rustc_driver::run_compiler(&args, callbacks);
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
            "-Zsensitivity-high" => config.report_level = ReportLevel::Error,
            "-Zsensitivity-med" => config.report_level = ReportLevel::Warning,
            "-Zsensitivity-low" => config.report_level = ReportLevel::Info,
            _ => {
                rustc_args.push(arg);
            }
        }
    }

    (config, rustc_args)
}

fn main() {
    // ICE: Internal Compilation Error
    rustc_driver::install_ice_hook(
        "https://github.com/AntDroid/Rudra/issues/new",
        |_| {},
    );

    let exit_code = {
        // initialize the report logger
        // `logger_handle` must be nested because it flushes the logs when it goes out of the scope
        let (config, mut rustc_args) = parse_config();
        let _logger_handle = init_report_logger(default_report_logger());

        // init rustc logger
        let early_dcx = EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        if env::var_os("RUSTC_LOG").is_some() {
            rustc_driver::init_rustc_env_logger(&early_dcx);
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
