use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use semver::Version;
use serde::Serialize;

use crawl::error::Result;
use crawl::krate::Crate;
use crawl::stat::CrateStat;
use crawl::ScratchDir;

#[derive(Serialize)]
struct CsvEntry {
    inaccuate: bool,
    name: String,
    version: Version,
    id: u64,
    downloads: u64,
    tokei: (),
    total_line: usize,
    blank_line: usize,
    code_line: usize,
    comment_line: usize,
    syn: (),
    num_fn: usize,
    num_unsafe_fn: usize,
    num_contains_unsafe_fn: usize,
    num_unsafe_global: usize,
}

impl From<&(Crate, CrateStat)> for CsvEntry {
    fn from((krate, stat): &(Crate, CrateStat)) -> Self {
        CsvEntry {
            inaccuate: stat.summary.inaccurate,
            name: krate.name().to_owned(),
            version: krate.latest_version_record().num.clone(),
            id: krate.id(),
            downloads: krate.downloads(),
            tokei: (),
            total_line: stat.summary.total_line,
            blank_line: stat.summary.blank_line,
            code_line: stat.summary.code_line,
            comment_line: stat.summary.comment_line,
            syn: (),
            num_fn: stat.summary.num_fn,
            num_unsafe_fn: stat.summary.num_unsafe_fn,
            num_contains_unsafe_fn: stat.summary.num_contains_unsafe_fn,
            num_unsafe_global: stat.summary.num_unsafe_global,
        }
    }
}

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

fn print_csv(file_name: impl AsRef<Path>, crate_list: &Vec<(Crate, CrateStat)>) -> Result<()> {
    let file = File::create(file_name)?;
    let buf_writer = BufWriter::new(file);
    let mut csv_writer = csv::Writer::from_writer(buf_writer);
    for record in crate_list.iter() {
        csv_writer.serialize::<CsvEntry>(record.into())?;
    }

    Ok(())
}

fn main() -> Result<()> {
    setup_log();

    let scratch_dir = ScratchDir::new();

    let crate_list = scratch_dir.fetch_crate_info()?;
    // TODO: increase the number of crates
    let crate_list: Vec<_> = crate_list.into_iter().take(5).collect();
    let crate_list: Vec<_> = crate_list
        .into_iter()
        .map(|krate| -> Result<(Crate, CrateStat)> {
            let path = scratch_dir.fetch_latest_version(&krate)?;
            let crate_stat = crawl::stat::stat(&path)?;
            Ok((krate, crate_stat))
        })
        .filter_map(|result| result.ok())
        .collect();

    print_csv("unsafe-counter.csv", &crate_list)?;

    Ok(())
}
