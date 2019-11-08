use rustc::ty::Instance;

#[derive(Debug)]
pub enum AnalysisError<'tcx> {
    NoMirForInstance(Instance<'tcx>),
    Unsupported(String),
    InvalidReturnContent,
}

pub type StepResult<'tcx> = Result<(), AnalysisError<'tcx>>;
