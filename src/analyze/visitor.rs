use rustc::mir;
use rustc::ty::{Instance, TyCtxt};

use super::{AnalysisContext, AnalysisError, LocationContent, StepResult};
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
                    return Err(AnalysisError::Unimplemented(
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
        _tcx: TyCtxt<'tcx>,
        acx: &mut AnalysisContext,
        statement: &mir::Statement<'tcx>,
    ) -> StepResult<'tcx> {
        use mir::StatementKind::*;
        match statement.kind {
            Assign(box (ref dst, ref rvalue)) => {
                use mir::Rvalue::*;
                match rvalue {
                    Use(ref operand) => acx.handle_assign(dst, operand)?,

                    // this code doesn't consider borrow kind
                    Ref(_, _, ref src) => acx.handle_ref(dst, src)?,

                    BinaryOp(_, _, _) | CheckedBinaryOp(_, _, _) | UnaryOp(_, _) => {
                        acx.update_location(acx.resolve_place(dst)?, LocationContent::Value)?;
                    }

                    // TODO: support more rvalue
                    _ => {
                        return Err(AnalysisError::Unimplemented(
                            "Unimplemented rvalue".to_owned(),
                            Some(statement.source_info.span),
                        ))
                    }
                }
            }

            // TODO: take a look at stacked borrow model, then decide whether or not to implement this
            Retag(_, _) => (),

            // NOP
            StorageLive(_) | StorageDead(_) | Nop => (),

            // TODO: support more statements
            _ => {
                return Err(AnalysisError::Unimplemented(
                    "Unimplemented statement".to_owned(),
                    Some(statement.source_info.span),
                ));
            }
        }

        Ok(())
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

            // TODO: support more terminators
            _ => Continuation::Unimplemented,
        };
        Ok(continuation)
    }
}
