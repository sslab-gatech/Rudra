use rustc::ty::Instance;

#[derive(Debug)]
pub enum Error<'tcx> {
    BodyNotAvailable(Instance<'tcx>),
    Unimplemented(String),
}

pub type Result<'tcx> = std::result::Result<(), Error<'tcx>>;
