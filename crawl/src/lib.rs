#![feature(backtrace)]

pub mod error;
pub mod krate;
pub mod stat;

use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use flate2::read::GzDecoder;
use log::*;
use once_cell::sync::Lazy;
use rand::Rng;
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

const ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);

fn download(url: impl IntoUrl, path: impl AsRef<Path>) -> Result<()> {
    // Read Crawler policies for crates.io here: https://crates.io/policies
    let sleep_duration = rand::thread_rng().gen_range(8_000, 16_000);
    thread::sleep(Duration::from_millis(sleep_duration));

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

fn file_needs_refresh(path: impl AsRef<Path>, duration: Duration) -> bool {
    let path = path.as_ref();

    if path.exists() {
        let metadata = path.metadata().expect("failed to access metadata");
        let modified_time = metadata.modified().expect("failed to read modified time");
        let diff = SystemTime::now().duration_since(modified_time).unwrap();
        diff >= duration
    } else {
        true
    }
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

pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    pub fn new() -> Self {
        let path = PathBuf::from(env::var("CRUX_SCRATCH").unwrap_or(String::from("./scratch")));
        info!("Using `{}` as scratch directory", path.to_string_lossy());
        ScratchDir { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn fetch_crate_info(&self) -> Result<Vec<Crate>> {
        let db_dump_dir = self.path.join("db-dump");
        let db_dump_tarball = self.path.join("db-dump.tar.gz");
        if file_needs_refresh(&db_dump_tarball, ONE_DAY) {
            info!("Start downloading new DB");
            download("https://static.crates.io/db-dump.tar.gz", &db_dump_tarball)?;

            fs::remove_dir_all(&db_dump_dir).ok();
            fs::create_dir(&db_dump_dir)?;
            decompress(&db_dump_tarball, &db_dump_dir)?;
        } else {
            info!("Use existing DB");
        }

        let unpacked_path = fs::read_dir(&db_dump_dir)?.next().unwrap()?.path();
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
            info!("Fetching `{}`", &version_tag);
            download(
                &format!(
                    "https://static.crates.io/crates/{}/{}.crate",
                    krate.name(),
                    version_tag,
                ),
                &crate_path,
            )?;
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
