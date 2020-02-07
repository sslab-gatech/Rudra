use std::env;
use std::path::PathBuf;

use log::*;

use crawl::error::Result;
use crawl::fetch_crate_info;

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

    let scratch_dir = PathBuf::from(env::var("CRUX_SCRATCH").unwrap_or(String::from("./scratch")));
    info!(
        "Using `{}` as scratch directory",
        scratch_dir.to_string_lossy()
    );

    let _crate_list = fetch_crate_info(&scratch_dir)?;

    Ok(())
}
