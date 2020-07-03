use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use std::borrow::Cow;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::Deref;
use std::path::PathBuf;

use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use serde::Serialize;

static REPORT_LOGGER: OnceCell<Box<dyn ReportLogger>> = OnceCell::new();

/// Flushes the global report logger when dropped.
pub struct FlushHandle {
    _priv: (),
}

impl Drop for FlushHandle {
    fn drop(&mut self) {
        for logger in REPORT_LOGGER.get().iter() {
            logger.flush();
        }
    }
}

#[must_use]
pub fn init_report_logger(report_logger: Box<dyn ReportLogger>) -> FlushHandle {
    REPORT_LOGGER
        .set(report_logger)
        .map_err(|_| ())
        .expect("The logger is already initialized");

    FlushHandle { _priv: () }
}

pub fn default_report_logger() -> Box<dyn ReportLogger> {
    match env::var_os("CRUX_REPORT") {
        Some(val) => Box::new(FileLogger::new(val)),
        None => Box::new(StderrLogger::new()),
    }
}

pub fn crux_report(report: Report) {
    REPORT_LOGGER.get().unwrap().log(report);
}

#[derive(Serialize, Debug)]
pub enum ReportLevel {
    Error,
    Warning,
    Info,
}

impl fmt::Display for ReportLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Serialize)]
pub struct Report {
    level: ReportLevel,
    analyzer: Cow<'static, str>,
    description: Cow<'static, str>,
    location: String,
    source: String,
}

impl Report {
    pub fn with_span<T, U>(
        tcx: TyCtxt<'_>,
        level: ReportLevel,
        analyzer: T,
        description: U,
        span: Span,
    ) -> Report
    where
        T: Into<Cow<'static, str>>,
        U: Into<Cow<'static, str>>,
    {
        let source_map = tcx.sess.source_map();
        let location = source_map.span_to_string(span);
        let source = source_map
            .span_to_snippet(span)
            .unwrap_or_else(|e| format!("unable to get source: {:?}", e));

        Report {
            level,
            analyzer: analyzer.into(),
            description: description.into(),
            location,
            source,
        }
    }
}

pub trait ReportLogger: Sync + Send {
    fn log(&self, report: Report);
    fn flush(&self);
}

struct StderrLogger {
    reports: Mutex<Vec<Report>>,
}

impl StderrLogger {
    fn new() -> Self {
        StderrLogger {
            reports: Mutex::new(Vec::new()),
        }
    }
}

impl ReportLogger for StderrLogger {
    fn log(&self, report: Report) {
        self.reports.lock().push(report);
    }

    fn flush(&self) {
        let stderr = std::io::stderr();
        let mut handle = stderr.lock();

        let reports = self.reports.lock();
        for report in reports.iter() {
            writeln!(
                &mut handle,
                "{} ({}): {}\n-> {}\n{}",
                &report.level,
                &report.analyzer,
                &report.description,
                &report.location,
                &report.source
            )
            .expect("stderr closed");
        }
    }
}

struct FileLogger {
    reports: Mutex<Vec<Report>>,
    file_path: PathBuf,
}

impl FileLogger {
    fn new<T>(val: T) -> Self
    where
        T: Into<PathBuf>,
    {
        FileLogger {
            reports: Mutex::new(Vec::new()),
            file_path: val.into(),
        }
    }
}

impl ReportLogger for FileLogger {
    fn log(&self, report: Report) {
        self.reports.lock().push(report);
    }

    fn flush(&self) {
        let reports = self.reports.lock();
        if !reports.is_empty() {
            let file = File::create(&self.file_path).expect("failed to create Crux report file");
            let mut file = BufWriter::new(file);
            serde_json::ser::to_writer_pretty(&mut file, reports.deref())
                .expect("cannot write Crux report to file");
        }
    }
}
