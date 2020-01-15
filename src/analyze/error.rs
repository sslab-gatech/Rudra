use rustc::ty::Instance;
use rustc_span::Span;

#[derive(Debug)]
pub enum AnalysisError<'tcx> {
    BodyNotAvailable(Instance<'tcx>),
    Unimplemented(String, Option<Span>),
    InfiniteLoop,
    InvalidReturnContent(String),
    WriteToDeadLocation,
}

pub type StepResult<'tcx> = Result<(), AnalysisError<'tcx>>;
