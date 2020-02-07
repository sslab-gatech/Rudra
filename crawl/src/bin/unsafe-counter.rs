use std::env;

use log::*;

use crawl::error::Result;
use crawl::ScratchDir;

fn setup_log() {
    dotenv::dotenv().ok();
    let log_var_name = "CRUX_LOG";

    if let None = env::var_os(log_var_name) {
        env::set_var(
            log_var_name,
            "warn,crawl=info,unsafe_counter=info,tokei::language::language_type=error",
        );
    }
    pretty_env_logger::init_custom_env(log_var_name);
}

fn main() -> Result<()> {
    setup_log();

    let scratch_dir = ScratchDir::new();
    info!(
        "Using `{}` as scratch directory",
        scratch_dir.path().to_string_lossy()
    );

    let crate_list = scratch_dir.fetch_crate_info()?;

    for krate in crate_list.iter().take(5) {
        scratch_dir.fetch_latest_version(krate)?;
    }

    Ok(())
}
