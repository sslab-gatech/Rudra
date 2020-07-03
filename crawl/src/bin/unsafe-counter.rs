#![feature(try_blocks)]

use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use log::*;
use rayon::prelude::*;
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
    num_loop_in_unsafe_fn: usize,
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
            num_loop_in_unsafe_fn: stat.summary.num_loop_in_unsafe_fn,
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

fn setup_rayon() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(16)
        .stack_size(8 * 1024 * 1024) // syn requires bigger stack
        .build_global()
        .expect("Failed to initialize thread pool");
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
    setup_rayon();

    let scratch_dir = ScratchDir::new();

    let crate_list = scratch_dir.fetch_crate_info()?;
    let num_total = crate_list.len();

    let crate_list: Vec<_> = crate_list
        .into_par_iter()
        .filter_map(|krate| -> Option<(Crate, CrateStat)> {
            let result: Result<CrateStat> = try {
                let path = scratch_dir.fetch_latest_version(&krate)?;
                let crate_stat = crawl::stat::stat(&path)?;
                crate_stat
            };

            match result {
                Ok(crate_stat) => Some((krate, crate_stat)),
                Err(e) => {
                    warn!("{}: {}", krate.latest_version_tag(), &e);
                    None
                }
            }
        })
        .collect();
    let num_success = crate_list.len();

    let num_unsafe = crate_list
        .par_iter()
        .filter(|(_, stat)| {
            stat.summary.num_unsafe_fn > 0
                || stat.summary.num_contains_unsafe_fn > 0
                || stat.summary.num_unsafe_global > 0
        })
        .count();

    println!("Total: {}", num_total);
    println!("Success: {}", num_success);
    println!("Fail: {}", num_total - num_success);
    println!("Has Unsafe: {}", num_unsafe);

    print_csv("unsafe-counter.csv", &crate_list)?;

    Ok(())
}
