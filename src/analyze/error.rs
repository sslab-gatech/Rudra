use rustc::ty::Instance;
use syntax::source_map::Span;

#[derive(Debug)]
pub enum AnalysisError<'tcx> {
    NoMirForInstance(Instance<'tcx>),
    Unsupported(String, Option<Span>),
    InfiniteLoop,
    InvalidReturnContent,
}

pub type StepResult<'tcx> = Result<(), AnalysisError<'tcx>>;
