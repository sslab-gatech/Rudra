#![feature(backtrace)]

pub mod error;
pub mod krate;

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::time::{Duration, SystemTime};

use flate2::read::GzDecoder;
use log::*;
use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use reqwest::IntoUrl;
use serde::de::DeserializeOwned;
use tar::Archive;

use crate::error::Result;
use crate::krate::*;

// Read Crawler policies for crates.io here: https://crates.io/policies
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
    let file = File::create(path.as_ref())?;
    let mut buf_writer = BufWriter::new(file);
    CLIENT.get(url).send()?.copy_to(&mut buf_writer)?;

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

pub fn fetch_crate_info(scratch_dir: &Path) -> Result<Vec<Crate>> {
    let db_dump_dir = scratch_dir.join("db-dump");
    let db_dump_tarball = scratch_dir.join("db-dump.tar.gz");
    if file_needs_refresh(&db_dump_tarball, ONE_DAY) {
        info!("Start downloading new DB");
        download("https://static.crates.io/db-dump.tar.gz", &db_dump_tarball)?;

        let tar_gz = fs::File::open(&db_dump_tarball)?;
        let enc = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(enc);

        fs::remove_dir_all(&db_dump_dir).ok();
        fs::create_dir(&db_dump_dir)?;
        archive.unpack(&db_dump_dir)?;
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
