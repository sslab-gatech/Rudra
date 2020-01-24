mod error;
mod graph;

use rustc::ty::Instance;

pub use self::error::AnalysisError;
use self::graph::Scc;
pub use crate::prelude::*;

pub struct Analyzer<'ccx, 'tcx> {
    ccx: CruxCtxt<'ccx, 'tcx>,
}

impl<'ccx, 'tcx> Analyzer<'ccx, 'tcx> {
    pub fn new(ccx: CruxCtxt<'ccx, 'tcx>) -> Self {
        Analyzer { ccx }
    }

    pub fn analyze(&mut self, instance: Instance<'tcx>) -> Result<(), AnalysisError> {
        let body = self.ccx.instance_body(instance);
        let body = match &*body {
            Ok(body) => body,
            Err(_) => return Err(AnalysisError::BodyNotAvailable(instance)),
        };

        let scc = Scc::construct(body);
        let group_order = scc.topological_order();

        todo!()
    }
}
