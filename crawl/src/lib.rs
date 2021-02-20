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
use std::time::{Duration, Instant};

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

static DB_DUMP_DOWNLOAD_URL: &str = "https://github.com/Qwaz/crates.io-index-2020-07-04/releases/download/2020-07-04/db-dump.tar.gz";

static CLIENT: Lazy<Client> = Lazy::new(|| {
    use reqwest::header;

    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("rudra runner 0.1.0 (yechan@gatech.edu)"),
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

pub struct RudraHomeDir(PathBuf);

impl RudraHomeDir {
    pub fn from_env() -> Self {
        RudraHomeDir(PathBuf::from(
            env::var("RUDRA_RUNNER_HOME").expect("RUDRA_RUNNER_HOME is not set"),
        ))
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        RudraHomeDir(path.into())
    }

    // match these directory names with `setup_rudra_runner_home.py`

    pub fn cargo_home_dir(&self) -> PathBuf {
        self.0.join("cargo_home")
    }

    pub fn sccache_home_dir(&self) -> PathBuf {
        self.0.join("sccache_home")
    }

    pub fn rudra_cache_dir(&self) -> PathBuf {
        self.0.join("rudra_cache")
    }

    pub fn campaign_dir(&self) -> PathBuf {
        self.0.join("campaign")
    }
}

pub struct RudraCacheDir {
    path: PathBuf,
}

impl RudraCacheDir {
    pub fn new(home_dir: &RudraHomeDir) -> Self {
        let path = home_dir.rudra_cache_dir();
        info!("Using `{}` as Rudra cache directory", path.display());
        RudraCacheDir { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn fetch_crate_info(&self) -> Result<Vec<Crate>> {
        let db_dump_dir = self.path.join("db-dump");
        let db_dump_tarball = self.path.join("db-dump.tar.gz");
        if !db_dump_tarball.exists() {
            info!("Start downloading DB dump from {}", DB_DUMP_DOWNLOAD_URL);
            download(DB_DUMP_DOWNLOAD_URL, &db_dump_tarball)?;

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

pub struct CampaignDir {
    log_path: PathBuf,
    report_path: PathBuf,
}

impl CampaignDir {
    pub fn new(home_dir: &RudraHomeDir) -> Self {
        let parent_path = home_dir.campaign_dir();

        let dt: DateTime<Local> = Local::now();
        let parent_path = parent_path.join(dt.format("%Y%m%d_%H%M%S").to_string());
        fs::create_dir_all(&parent_path).expect("Failed to create campaign directory");

        let log_path = parent_path.join("log");
        let report_path = parent_path.join("report");
        fs::create_dir(&log_path).expect("Failed to create campaign directory");
        fs::create_dir(&report_path).expect("Failed to create campaign directory");

        info!(
            "Using `{}` as the campaign directory",
            parent_path.to_string_lossy()
        );
        CampaignDir {
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
