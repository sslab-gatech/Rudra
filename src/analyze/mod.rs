mod context;
mod error;

use rustc::ty::{Instance, TyCtxt};

use context::AnalysisContext;
pub use context::AnalysisSummary;
pub use error::AnalysisError;

pub struct Analyzer<'tcx> {
    _tcx: TyCtxt<'tcx>,
}

impl<'tcx> Analyzer<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Analyzer { _tcx: tcx }
    }

    pub fn analyze(&mut self, _entry: Instance<'tcx>) -> Result<AnalysisSummary, AnalysisError> {
        let acx = AnalysisContext::new();
        // TODO: perform an actual analysis
        Ok(acx.generate_summary())
    }
}
