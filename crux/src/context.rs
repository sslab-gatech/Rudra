use std::result::Result as StdResult;

use rustc::mir;
use rustc::ty::{Instance, TyCtxt, TyKind};

use dashmap::mapref::one::RefMut;
use dashmap::DashMap;

use crate::error::{Error, Result};
use crate::ir;
use crate::prelude::*;

macro_rules! unimplemented {
    () => (return Err(Error::TranslationUnimplemented(String::new())));
    ($($arg:tt)+) => (return Err(Error::TranslationUnimplemented(format!($($arg)+))));
}

pub type CruxCtxt<'ccx, 'tcx> = &'ccx CruxCtxtOwner<'tcx>;

/// Maps Instance to MIR and cache the result.
pub struct CruxCtxtOwner<'tcx> {
    tcx: TyCtxt<'tcx>,
    cache: DashMap<Instance<'tcx>, Result<'tcx, ir::Body<'tcx>>>,
}

/// Visit MIR body and returns a Crux IR function
/// Check rustc::mir::visit::Visitor for possible visit targets
/// https://doc.rust-lang.org/nightly/nightly-rustc/rustc/mir/visit/trait.Visitor.html
impl<'tcx> CruxCtxtOwner<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        CruxCtxtOwner {
            tcx,
            cache: DashMap::new(),
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn instance_body(
        &self,
        instance: Instance<'tcx>,
    ) -> RefMut<Instance<'tcx>, Result<'tcx, ir::Body<'tcx>>> {
        let tcx = self.tcx();
        let result = self.cache.entry(instance).or_insert_with(|| {
            let mir_body = tcx
                .find_fn(instance)
                .body()
                .ok_or_else(|| Error::BodyNotAvailable(instance))?;

            self.translate_body(instance, mir_body)
        });

        result
    }

    fn translate_body(
        &self,
        instance: Instance<'tcx>,
        body: &mir::Body<'tcx>,
    ) -> Result<'tcx, ir::Body<'tcx>> {
        let local_decls = body
            .local_decls
            .iter()
            .map(|local_decl| self.translate_local_decl(local_decl))
            .collect::<StdResult<Vec<_>, _>>()?;

        let basic_blocks: Vec<_> = body
            .basic_blocks()
            .iter()
            .map(|basic_block| self.translate_basic_block(instance, basic_block))
            .collect::<StdResult<Vec<_>, _>>()?;

        Ok(ir::Body {
            local_decls,
            original_decls: body.local_decls.clone(),
            basic_blocks,
        })
    }

    fn translate_basic_block(
        &self,
        instance: Instance<'tcx>,
        basic_block: &mir::BasicBlockData<'tcx>,
    ) -> Result<'tcx, ir::BasicBlock<'tcx>> {
        let statements = basic_block
            .statements
            .iter()
            .map(|statement| statement.clone())
            .collect::<Vec<_>>();

        let terminator = self.translate_terminator(
            instance,
            basic_block
                .terminator
                .as_ref()
                .expect("Terminator should not be empty at this point"),
        )?;

        Ok(ir::BasicBlock {
            statements,
            terminator,
            is_cleanup: basic_block.is_cleanup,
        })
    }

    fn translate_terminator(
        &self,
        instance: Instance<'tcx>,
        terminator: &mir::Terminator<'tcx>,
    ) -> Result<'tcx, ir::Terminator<'tcx>> {
        let caller_substs = instance.substs;

        use mir::TerminatorKind::*;
        Ok(ir::Terminator {
            kind: match &terminator.kind {
                Goto { target } => ir::TerminatorKind::Goto(target.index()),
                Return => ir::TerminatorKind::Return,
                Call {
                    func: func_operand,
                    args,
                    destination,
                    cleanup,
                    ..
                } => {
                    let cleanup = cleanup.clone().map(|block| block.index());
                    let destination = {
                        if let Some((place, block)) = destination {
                            (place.clone(), block.index())
                        } else {
                            unimplemented!("Diverging function call is not yet supported");
                        }
                    };

                    if let mir::Operand::Constant(box func) = func_operand {
                        let func_ty = func.literal.ty;
                        match func_ty.kind {
                            TyKind::FnDef(def_id, callee_substs) => {
                                let instance = self
                                    .tcx()
                                    .monomorphic_resolve(def_id, callee_substs, caller_substs)
                                    .expect("Unexpected resolve failure");
                                ir::TerminatorKind::StaticCall {
                                    target: instance,
                                    args: args.clone(),
                                    cleanup,
                                    destination,
                                }
                            }
                            TyKind::FnPtr(_) => {
                                unimplemented!("Call through function ptr is not yet supported")
                            }
                            _ => panic!("invalid callee of type {:?}", func_ty),
                        }
                    } else {
                        unimplemented!("Non-constant function call is not supported")
                    }
                }
                _ => unimplemented!("Unknown terminator: {:?}", terminator),
            },
        })
    }

    fn translate_local_decl(
        &self,
        local_decl: &mir::LocalDecl<'tcx>,
    ) -> Result<'tcx, ir::LocalDecl<'tcx>> {
        Ok(ir::LocalDecl { ty: local_decl.ty })
    }
}
