#![feature(backtrace)]

pub mod error;
pub mod krate;
pub mod stat;
pub mod utils;

use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use chrono::prelude::*;
use flate2::read::GzDecoder;
use log::*;
use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use reqwest::IntoUrl;
use serde::de::DeserializeOwned;
use tar::Archive;

use crate::error::Result;
use crate::krate::*;

static CLIENT: Lazy<Client> = Lazy::new(|| {
    use reqwest::header;

    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("crux crawler 0.1.0 (yechan@gatech.edu)"),
    );

    Client::builder()
        .default_headers(headers)
        .build()
        .expect("Failed to build reqwest client")
});

static LAST_DOWNLOAD: Lazy<Mutex<Instant>> = Lazy::new(|| Mutex::new(Instant::now()));

fn download(url: impl IntoUrl, path: impl AsRef<Path>) -> Result<()> {
    const REQUIRED_DELAY: Duration = Duration::from_secs(1);

    // Read Crawler policies for crates.io here: https://crates.io/policies
    let mut last_download = LAST_DOWNLOAD.lock().unwrap();
    let mut now = Instant::now();

    let diff = now.duration_since(*last_download);
    if diff < REQUIRED_DELAY {
        thread::sleep(REQUIRED_DELAY - diff);
        now = Instant::now();
    }

    *last_download = now;
    drop(last_download);

    let file = File::create(path.as_ref())?;
    let mut buf_writer = BufWriter::new(file);
    CLIENT.get(url).send()?.copy_to(&mut buf_writer)?;

    Ok(())
}

fn decompress(tarball_path: &Path, parent_path: &Path) -> Result<()> {
    let tar_gz = fs::File::open(&tarball_path)?;
    let enc = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(enc);

    archive.unpack(&parent_path)?;

    Ok(())
}

fn parse_csv_records<T: DeserializeOwned>(csv_path: &Path) -> Result<Vec<T>> {
    let file = File::open(csv_path)?;
    let buf_reader = BufReader::new(file);
    let mut csv_reader = csv::Reader::from_reader(buf_reader);

    let mut vec = Vec::new();
    for result in csv_reader.deserialize() {
        let record = result?;
        vec.push(record);
    }

    Ok(vec)
}

pub fn refresh_never(_path: &Path) -> bool {
    false
}

pub fn refresh_everyday(path: &Path) -> bool {
    const ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);

    let metadata = path.metadata().expect("failed to access metadata");
    let modified_time = metadata.modified().expect("failed to read modified time");
    let diff = SystemTime::now().duration_since(modified_time).unwrap();
    diff >= ONE_DAY
}

pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    pub fn new() -> Self {
        let path =
            PathBuf::from(env::var("CRUX_SCRATCH_DIR").unwrap_or(String::from("../crux_scratch")));
        info!("Using `{}` as scratch directory", path.to_string_lossy());
        ScratchDir { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn fetch_crate_info<F>(&self, refresh_criteria: F) -> Result<Vec<Crate>>
    where
        F: FnOnce(&Path) -> bool,
    {
        let db_dump_dir = self.path.join("db-dump");
        let db_dump_tarball = self.path.join("db-dump.tar.gz");
        if !db_dump_tarball.exists() || refresh_criteria(&db_dump_tarball) {
            info!("Start downloading new DB");
            download("https://static.crates.io/db-dump.tar.gz", &db_dump_tarball)?;

            fs::remove_dir_all(&db_dump_dir).ok();
            fs::create_dir(&db_dump_dir).expect("failed to create DB dump directory");
            decompress(&db_dump_tarball, &db_dump_dir).expect("DB decompression failed");
        } else {
            info!("Use existing DB");
        }

        let unpacked_path = fs::read_dir(&db_dump_dir)
            .expect("DB dump directory does not exist")
            .next()
            .expect("DB dump directory is empty")
            .expect("Failed to read DB dump directory")
            .path();
        info!(
            "Database version: {}",
            unpacked_path
                .components()
                .last()
                .unwrap()
                .as_os_str()
                .to_string_lossy()
        );

        let crate_list = parse_csv_records::<CrateRecord>(&unpacked_path.join("data/crates.csv"))?;
        let mut map = HashMap::with_capacity(crate_list.len());
        for crate_record in crate_list.iter() {
            map.insert(crate_record.id, Vec::new());
        }

        let version_list =
            parse_csv_records::<VersionRecord>(&unpacked_path.join("data/versions.csv"))?;
        for version_record in version_list.into_iter() {
            map.get_mut(&version_record.crate_id)
                .unwrap()
                .push(version_record);
        }

        let mut vec = Vec::new();
        for crate_record in crate_list.into_iter() {
            vec.push(Crate::new(
                crate_record.clone(),
                map.remove(&crate_record.id).unwrap(),
            ));
        }

        Ok(vec)
    }

    pub fn fetch_latest_version(&self, krate: &Crate) -> Result<PathBuf> {
        let version_tag = krate.latest_version_tag();

        // download .crate file
        let crate_path = self.path.join(format!("{}.crate", &version_tag));
        if !crate_path.exists() {
            download(
                &format!(
                    "https://static.crates.io/crates/{}/{}.crate",
                    krate.name(),
                    version_tag,
                ),
                &crate_path,
            )?;
            info!("Downloaded `{}`", &version_tag);
        }

        // unpack .crate file
        let crate_content_path = self.path.join(&version_tag);
        if !crate_content_path.exists() {
            info!("Unpacking `{}`", &version_tag);
            decompress(&crate_path, &self.path)?;
        } else {
            debug!("Use existing `{}`", &version_tag);
        }

        Ok(crate_content_path)
    }
}

pub struct ReportDir {
    log_path: PathBuf,
    report_path: PathBuf,
}

impl ReportDir {
    pub fn new() -> Self {
        let parent_path =
            PathBuf::from(env::var("CRUX_REPORT_DIR").unwrap_or(String::from("../crux_report")));

        let dt: DateTime<Local> = Local::now();
        let parent_path = parent_path.join(dt.format("%Y%m%d_%H%M%S").to_string());
        fs::create_dir_all(&parent_path).expect("Failed to create report directory");

        let log_path = parent_path.join("log");
        let report_path = parent_path.join("report");
        fs::create_dir(&log_path).expect("Failed to create report directory");
        fs::create_dir(&report_path).expect("Failed to create report directory");

        info!(
            "Using `{}` as report directory",
            parent_path.to_string_lossy()
        );
        ReportDir {
            log_path,
            report_path,
        }
    }

    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    pub fn report_path(&self) -> &PathBuf {
        &self.report_path
    }
}
