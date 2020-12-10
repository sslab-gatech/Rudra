use rustc_hir::hir_id::HirId;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use std::borrow::Cow;
use std::env;
use std::fmt;
use std::fs;
use std::io::Write;
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
    match env::var_os("RUDRA_REPORT_PATH") {
        Some(val) => Box::new(FileLogger::new(val)),
        None => Box::new(StderrLogger::new()),
    }
}

pub fn rudra_report(report: Report) {
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
        item_hir_id: HirId,
    ) -> Report
    where
        T: Into<Cow<'static, str>>,
        U: Into<Cow<'static, str>>,
    {
        let source_map = tcx.sess.source_map();
        let source = if span.from_expansion() {
            let map = tcx.hir();
            // User-Friendly report for macro-generated code
            rustc_hir_pretty::to_string(map.krate(), |state| {
                state.print_item(map.item(item_hir_id));
            })
        } else {
            source_map
                .span_to_snippet(span)
                .unwrap_or_else(|e| format!("unable to get source: {:?}", e))
        };
        let location = source_map.span_to_string(span);

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
        #[derive(Serialize)]
        struct Reports<'a> {
            reports: &'a [Report],
        }

        let reports = self.reports.lock();
        if !reports.is_empty() {
            let reports_ref = &*reports;
            fs::write(
                &self.file_path,
                toml::to_string_pretty(&Reports {
                    reports: reports_ref,
                })
                .expect("failed to serialize Rudra report"),
            )
            .expect("cannot write Rudra report to file");
        }
    }
}
