use rustc::ty::{Instance, TyCtxt};

use super::{AnalysisContext, AnalysisError, StepResult};
use crate::TyCtxtExt;

pub struct CruxVisitor {}

// Check rustc::mir::visit::Visitor for possible visit targets
// https://doc.rust-lang.org/nightly/nightly-rustc/rustc/mir/visit/trait.Visitor.html
impl CruxVisitor {
    pub fn new() -> Self {
        CruxVisitor {}
    }

    pub fn visit_instance<'tcx>(
        &mut self,
        tcx: TyCtxt<'tcx>,
        acx: &mut AnalysisContext,
        instance: Instance<'tcx>,
    ) -> StepResult<'tcx> {
        let body = tcx
            .find_fn(instance)
            .ok_or_else(|| AnalysisError::NoMirForInstance(instance))?;

        acx.enter_body(body)?;

        // TODO: handle statements in the body

        acx.exit_body(body)?;

        Ok(())
    }
}
