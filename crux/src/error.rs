use rustc_middle::ty::Instance;

#[derive(Clone, Debug)]
pub enum Error<'tcx> {
    BodyNotAvailable(Instance<'tcx>),
    AnalysisUnimplemented(String),
    TranslationUnimplemented(String),
}

pub type Result<'tcx, T> = std::result::Result<T, Error<'tcx>>;

#[macro_export]
macro_rules! not_yet {
    () => (return Err(Error::AnalysisUnimplemented(String::new())));
    ($($arg:tt)+) => (return Err(Error::AnalysisUnimplemented(format!($($arg)+))));
}
