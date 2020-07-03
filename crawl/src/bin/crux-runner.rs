#![feature(try_blocks)]

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use log::*;
use rayon::prelude::*;

use crawl::error::Result;
use crawl::krate::Crate;
use crawl::stat::CrateStat;
use crawl::utils::*;
use crawl::{ReportDir, ScratchDir};

fn setup_log() {
    dotenv::dotenv().ok();
    let log_var_name = "CRUX_LOG";

    if let None = env::var_os(log_var_name) {
        env::set_var(
            log_var_name,
            "warn,crawl=info,crux_runner=info,tokei::language::language_type=error",
        );
    }
    pretty_env_logger::init_custom_env(log_var_name);
}

fn setup_rayon() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(16)
        .stack_size(8 * 1024 * 1024)
        .build_global()
        .expect("Failed to initialize thread pool");
}

fn main() -> Result<()> {
    setup_log();
    setup_rayon();

    let scratch_dir = ScratchDir::new();
    let report_dir = ReportDir::new();

    let crate_list = scratch_dir.fetch_crate_info()?;

    // first stage - fetching crate
    let crate_list: Vec<_> = crate_list
        .into_par_iter()
        // FIXME: experimental setup
        .take(1000)
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

    // second stage - run crux on them
    let _crate_list: Vec<_> = crate_list
        .into_par_iter()
        // TODO: performance optimization with unsafe filtering (CRUX-53)
        .filter_map(|(krate, path, _crate_stat)| -> Option<Crate> {
            // FIXME: add timeout (CRUX-43)
            info!("Analysis start: {}", krate.latest_version_tag());
            let crux_output = run_command_with_env(
                "cargo crux",
                &path,
                &[(
                    "CRUX_REPORT",
                    report_dir
                        .report_path()
                        .join(format!("report-{}", krate.latest_version_tag())),
                )],
            );
            info!("Analysis end: {}", krate.latest_version_tag());

            let clean_output = run_command("cargo clean", &path);
            if !is_cmd_success(&clean_output) {
                warn!("Failed to clean {}", krate.latest_version_tag());
            }

            let log_path = report_dir
                .log_path()
                .join(format!("log-{}", krate.latest_version_tag()));

            match crux_output {
                Ok(output) => {
                    if let Ok(file) = File::create(&log_path) {
                        let mut file = BufWriter::new(file);
                        write!(
                            &mut file,
                            "=== stdout ===\n{}\n=== stderr ===\n{}\n",
                            String::from_utf8_lossy(&output.stdout),
                            String::from_utf8_lossy(&output.stderr),
                        )
                        .ok();
                    } else {
                        error!("Failed to create {:?}", &log_path);
                    }
                    Some(krate)
                }
                Err(e) => {
                    error!(
                        "Failed to execute `cargo crux` on {}: {:?}",
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
