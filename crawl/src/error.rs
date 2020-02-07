use std::backtrace::Backtrace;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Initialization failed")]
    InitFail,
    #[error("External command `{0}` failed")]
    CommandFail(String),
    #[error("Crate Json is malformed")]
    MalformedCrateJson,
    #[error("Crate does not contain any content")]
    EmptyCrateError,
    #[error("No Rust file exists in the directory")]
    NoRustFileError,
    #[error("I/O error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
        backtrace: Backtrace,
    },
    #[error("CSV error: {source}")]
    CsvError {
        #[from]
        source: csv::Error,
        backtrace: Backtrace,
    },
    #[error("HTTP error: {source}")]
    ReqwestError {
        #[from]
        source: reqwest::Error,
        backtrace: Backtrace,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
