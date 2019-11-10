use rustc::ty::Instance;
use syntax::source_map::Span;

#[derive(Debug)]
pub enum AnalysisError<'tcx> {
    NoMirForInstance(Instance<'tcx>),
    Unimplemented(String, Option<Span>),
    InfiniteLoop,
    InvalidReturnContent,
    WriteToDeadLocation,
}

pub type StepResult<'tcx> = Result<(), AnalysisError<'tcx>>;
