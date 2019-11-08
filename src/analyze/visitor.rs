use rustc::mir;
use rustc::ty::{Instance, TyCtxt};

use super::{AnalysisContext, AnalysisError, StepResult};
use crate::TyCtxtExt;

pub struct CruxVisitor {}

enum Continuation {
    Unimplemented,
    Goto(mir::BasicBlock),
    Return,
}

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

        // TODO: proper infinite loop handling instead of counter
        let mut loop_counter = 500;
        let mut next_block = &body.basic_blocks().raw[0];
        while loop_counter > 0 {
            let continuation = self.visit_basic_block(tcx, acx, next_block)?;
            match continuation {
                Continuation::Goto(basic_block) => next_block = &body.basic_blocks()[basic_block],
                Continuation::Return => break,
                Continuation::Unimplemented => {
                    return Err(AnalysisError::Unsupported(
                        "The function used unsupported continuation".to_owned(),
                        Some(body.span),
                    ))
                }
            }
            loop_counter -= 1;
        }
        if loop_counter == 0 {
            return Err(AnalysisError::InfiniteLoop);
        }

        acx.exit_body(body)?;

        Ok(())
    }

    fn visit_basic_block<'tcx>(
        &mut self,
        tcx: TyCtxt<'tcx>,
        acx: &mut AnalysisContext,
        basic_block: &mir::BasicBlockData<'tcx>,
    ) -> Result<Continuation, AnalysisError<'tcx>> {
        for statment in basic_block.statements.iter() {
            self.visit_statement(tcx, acx, statment)?;
        }

        self.visit_terminator(basic_block.terminator.as_ref().unwrap())
    }

    fn visit_statement<'tcx>(
        &mut self,
        tcx: TyCtxt<'tcx>,
        acx: &mut AnalysisContext,
        statement: &mir::Statement<'tcx>,
    ) -> StepResult<'tcx> {
        unimplemented!()
    }

    fn visit_terminator<'tcx>(
        &mut self,
        terminator: &mir::Terminator<'tcx>,
    ) -> Result<Continuation, AnalysisError<'tcx>> {
        use rustc::mir::TerminatorKind::*;
        let continuation = match terminator.kind {
            Goto { target } => Continuation::Goto(target),
            FalseEdges { real_target, .. } => Continuation::Goto(real_target),
            FalseUnwind { real_target, .. } => Continuation::Goto(real_target),

            Return => Continuation::Return,

            _ => Continuation::Unimplemented,
        };
        Ok(continuation)
    }
}
