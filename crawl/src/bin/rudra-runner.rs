use std::env;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use log::*;
use rayon::prelude::*;
use structopt::{clap::arg_enum, StructOpt};

use crawl::error::Result;
use crawl::krate::Crate;
use crawl::utils::*;
use crawl::{CampaignDir, RudraCacheDir, RudraHomeDir};

arg_enum! {
    #[derive(Debug)]
    enum Selection {
        First,
        Top,
        Random,
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "rudra-runner", about = "Run Rudra on crates.io")]
struct Opt {
    #[structopt(short, long, possible_values = &Selection::variants(), case_insensitive = true, default_value = "first")]
    select: Selection,

    #[structopt(short = "n")]
    count: Option<usize>,
}

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
    let opt = Opt::from_args();

    setup_logging();
    setup_rayon();

    let rudra_home_dir = RudraHomeDir::from_env();
    let rudra_cache_dir = RudraCacheDir::new(&rudra_home_dir);
    let campaign_dir = CampaignDir::new(&rudra_home_dir);

    let crate_list = rudra_cache_dir.fetch_crate_info()?;

    // first stage - fetching crate
    // Add `.take(val)` after `.into_par_iter()` for a quick local test
    let mut crate_list: Vec<_> = crate_list
        .into_par_iter()
        .filter_map(|krate| -> Option<(Crate, PathBuf)> {
            match rudra_cache_dir.fetch_latest_version(&krate) {
                Ok(path) => Some((krate, path)),
                Err(e) => {
                    warn!("{}: {}", krate.latest_version_tag(), &e);
                    None
                }
            }
        })
        .collect();

    match opt.select {
        Selection::First => {
            crate_list.sort_by_key(|krate| krate.0.name().to_owned());
        }
        Selection::Top => {
            crate_list
                .sort_by_key(|krate| std::u64::MAX - krate.0.latest_version_record().downloads);
        }
        Selection::Random => {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            crate_list.as_mut_slice().shuffle(&mut rng);
        }
    }

    if let Some(count) = opt.count {
        crate_list.truncate(count)
    }

    // second stage - run rudra on them
    let _crate_list: Vec<_> = crate_list
        .into_par_iter()
        .filter_map(|(krate, path)| -> Option<Crate> {
            info!("Analysis start: {}", krate.latest_version_tag());

            let report_path = campaign_dir
                .report_path()
                .join(format!("report-{}", krate.latest_version_tag()));

            let log_path = campaign_dir
                .log_path()
                .join(format!("log-{}", krate.latest_version_tag()));

            let rudra_output = run_command_with_env(
                "cargo rudra -Zno-index-update --locked -j 1",
                &path,
                &[
                    ("RUSTUP_TOOLCHAIN", OsStr::new("nightly-2021-10-20")),
                    ("CARGO_HOME", rudra_home_dir.cargo_home_dir().as_ref()),
                    ("SCCACHE_DIR", rudra_home_dir.sccache_home_dir().as_ref()),
                    ("SCCACHE_CACHE_SIZE", "10T".as_ref()),
                    ("RUDRA_REPORT_PATH", report_path.as_ref()),
                    ("RUDRA_LOG_PATH", log_path.as_ref()),
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
