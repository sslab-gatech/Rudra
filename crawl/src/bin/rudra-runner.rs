#![feature(try_blocks)]

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use log::*;
use rayon::prelude::*;

use crawl::error::Result;
use crawl::krate::Crate;
use crawl::stat::CrateStat;
use crawl::utils::*;
use crawl::{refresh_never, ReportDir, ScratchDir};

fn setup_logging() {
    dotenv::dotenv().ok();
    let log_var_name = "RUDRA_RUNNER_LOG";

    if let None = env::var_os(log_var_name) {
        env::set_var(log_var_name, "info");
    }
    pretty_env_logger::init_custom_env(log_var_name);
}

fn setup_rayon() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_cpus::get())
        .stack_size(8 * 1024 * 1024)
        .build_global()
        .expect("Failed to initialize thread pool");
}

fn main() -> Result<()> {
    setup_logging();
    setup_rayon();

    let scratch_dir = ScratchDir::new();
    let report_dir = ReportDir::new();

    let crate_list = scratch_dir.fetch_crate_info(refresh_never)?;

    // first stage - fetching crate
    // Add `.take(val)` after `.into_par_iter()` for a quick local test
    let crate_list: Vec<_> = crate_list
        .into_par_iter()
        .filter_map(|krate| -> Option<(Crate, PathBuf, CrateStat)> {
            let result: Result<(PathBuf, CrateStat)> = try {
                let path = scratch_dir.fetch_latest_version(&krate)?;
                let crate_stat = crawl::stat::stat(&path)?;
                (path, crate_stat)
            };

            match result {
                Ok((path, crate_stat)) => Some((krate, path, crate_stat)),
                Err(e) => {
                    warn!("{}: {}", krate.latest_version_tag(), &e);
                    None
                }
            }
        })
        .collect();

    // second stage - run rudra on them
    let _crate_list: Vec<_> = crate_list
        .into_par_iter()
        // TODO: performance optimization with unsafe filtering (RUDRA-53)
        .filter_map(|(krate, path, _crate_stat)| -> Option<Crate> {
            // FIXME: add timeout (RUDRA-43)
            info!("Analysis start: {}", krate.latest_version_tag());

            let report_path = report_dir
                .report_path()
                .join(format!("report-{}", krate.latest_version_tag()));

            let log_path = report_dir
                .log_path()
                .join(format!("log-{}", krate.latest_version_tag()));

            let rudra_output = run_command_with_env(
                "cargo rudra",
                &path,
                &[
                    ("RUDRA_REPORT_PATH", &report_path),
                    ("RUDRA_LOG_PATH", &log_path),
                ],
            );
            info!("Analysis end: {}", krate.latest_version_tag());

            let clean_output = run_command("cargo clean", &path);
            if !is_cmd_success(&clean_output) {
                warn!("Failed to clean {}", krate.latest_version_tag());
            }

            match rudra_output {
                Ok(output) => {
                    let log_file = OpenOptions::new().append(true).create(true).open(&log_path);
                    if let Ok(mut file) = log_file {
                        if let Err(e) = write!(
                            &mut file,
                            "[stdout]\n{}\n[stderr]\n{}\n",
                            String::from_utf8_lossy(&output.stdout),
                            String::from_utf8_lossy(&output.stderr),
                        ) {
                            error!(
                                "Failed to write the log for {}: {}",
                                krate.latest_version_tag(),
                                e
                            );
                        }
                    } else {
                        error!("Failed to create {:?}", &log_path);
                    }
                    Some(krate)
                }
                Err(e) => {
                    error!(
                        "Failed to execute `cargo rudra` on {}: {}",
                        krate.latest_version_tag(),
                        e
                    );
                    None
                }
            }
        })
        .collect();

    Ok(())
}
