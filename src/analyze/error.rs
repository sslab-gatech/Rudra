use rustc::ty::Instance;

#[derive(Debug)]
pub enum AnalysisError<'tcx> {
    BodyNotAvailable(Instance<'tcx>),
    Unimplemented(String),
}
