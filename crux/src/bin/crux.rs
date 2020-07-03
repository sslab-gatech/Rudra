#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;

#[macro_use]
extern crate log;

use std::env;

use rustc_driver::Compilation;
use rustc_errors::ErrorReported;
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

fn main() -> Result<(), ErrorReported> {
    // init Crux logger
    dotenv().ok();
    let env = env_logger::Env::new()
        .filter("CRUX_LOG")
        .write_style("CRUX_LOG_STYLE");
    env_logger::init_from_env(env);

    // init rustc logger
    if env::var("RUSTC_LOG").is_ok() {
        rustc_driver::init_rustc_env_logger();
    }

    // init report logger
    let _logger_handle = init_report_logger(default_report_logger());

    // collect arguments
    let mut config = CruxAnalysisConfig::default();

    let mut after_dashdash = false;
    let mut rustc_args = vec![];
    let mut crux_args = vec![];

    for arg in std::env::args() {
        if rustc_args.is_empty() {
            // Very first arg: for `rustc`.
            rustc_args.push(arg);
        } else if after_dashdash {
            // Everything that comes after are `crux` args.
            crux_args.push(arg);
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
                "--" => {
                    after_dashdash = true;
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

    rustc_driver::install_ice_hook(); // ICE: Internal Compilation Error
    let result = rustc_driver::catch_fatal_errors(move || {
        rustc_driver::run_compiler(&rustc_args, &mut CruxCompilerCalls::new(config), None, None)
    })
    .and_then(|result| result);

    result
}
