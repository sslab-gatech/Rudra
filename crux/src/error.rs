use rustc::ty::Instance;

#[derive(Clone, Debug)]
pub enum Error<'tcx> {
    BodyNotAvailable(Instance<'tcx>),
    AnalysisUnimplemented(String),
    TranslationUnimplemented(String),
}

pub type Result<'tcx, T> = std::result::Result<T, Error<'tcx>>;
