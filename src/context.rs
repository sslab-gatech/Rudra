use std::rc::Rc;

use rustc_hir::{
    def_id::{DefId, LocalDefId},
    BodyId, ConstContext, HirId,
};
use rustc_middle::mir::{self, TerminatorKind};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::Span;

use dashmap::DashMap;
use snafu::Snafu;

use crate::ir;
use crate::prelude::*;
use crate::report::ReportLevel;
use crate::visitor::{create_adt_impl_map, AdtImplMap, RelatedFnCollector, RelatedItemMap};

#[derive(Debug, Snafu, Clone)]
pub enum MirInstantiationError {
    NotAvailable { def_id: DefId },
}

impl AnalysisError for MirInstantiationError {
    fn kind(&self) -> AnalysisErrorKind {
        use MirInstantiationError::*;
        match self {
            NotAvailable { .. } => AnalysisErrorKind::OutOfScope,
        }
    }
}

pub type RudraCtxt<'tcx> = &'tcx RudraCtxtOwner<'tcx>;
pub type TranslationResult<'tcx, T> = Result<T, MirInstantiationError>;

/// Maps Instance to MIR and cache the result.
pub struct RudraCtxtOwner<'tcx> {
    tcx: TyCtxt<'tcx>,
    translation_cache: DashMap<DefId, Rc<TranslationResult<'tcx, ir::Body<'tcx>>>>,
    related_item_cache: RelatedItemMap,
    adt_impl_cache: AdtImplMap<'tcx>,
    report_level: ReportLevel,
}

/// Visit MIR body and returns a Rudra IR function
/// Check rustc::mir::visit::Visitor for possible visit targets
/// https://doc.rust-lang.org/nightly/nightly-rustc/rustc/mir/visit/trait.Visitor.html
impl<'tcx> RudraCtxtOwner<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, report_level: ReportLevel) -> Self {
        RudraCtxtOwner {
            tcx,
            translation_cache: DashMap::new(),
            related_item_cache: RelatedFnCollector::collect(tcx),
            adt_impl_cache: create_adt_impl_map(tcx),
            report_level,
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn types_with_related_items(
        &self,
    ) -> impl Iterator<Item = (Option<HirId>, (BodyId, Span))> + '_ {
        (&self.related_item_cache)
            .into_iter()
            .flat_map(|(&k, v)| v.iter().map(move |&body_id| (k, body_id)))
    }

    pub fn translate_body(&self, def_id: DefId) -> Rc<TranslationResult<'tcx, ir::Body<'tcx>>> {
        let tcx = self.tcx();
        let result = self.translation_cache.entry(def_id).or_insert_with(|| {
            Rc::new(
                try {
                    let mir_body = Self::find_fn(tcx, def_id)?;
                    self.translate_body_impl(mir_body)?
                },
            )
        });

        result.clone()
    }

    fn translate_body_impl(
        &self,
        body: &mir::Body<'tcx>,
    ) -> TranslationResult<'tcx, ir::Body<'tcx>> {
        let local_decls = body
            .local_decls
            .iter()
            .map(|local_decl| self.translate_local_decl(local_decl))
            .collect::<Vec<_>>();

        let basic_blocks: Vec<_> = body
            .basic_blocks()
            .iter()
            .map(|basic_block| self.translate_basic_block(basic_block))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ir::Body {
            local_decls,
            original_decls: body.local_decls.to_owned(),
            basic_blocks,
            original: body.to_owned(),
        })
    }

    fn translate_basic_block(
        &self,
        basic_block: &mir::BasicBlockData<'tcx>,
    ) -> TranslationResult<'tcx, ir::BasicBlock<'tcx>> {
        let statements = basic_block
            .statements
            .iter()
            .map(|statement| statement.clone())
            .collect::<Vec<_>>();

        let terminator = self.translate_terminator(
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
        terminator: &mir::Terminator<'tcx>,
    ) -> TranslationResult<'tcx, ir::Terminator<'tcx>> {
        Ok(ir::Terminator {
            kind: match &terminator.kind {
                TerminatorKind::Goto { target } => ir::TerminatorKind::Goto(target.index()),
                TerminatorKind::Return => ir::TerminatorKind::Return,
                TerminatorKind::Call {
                    func: func_operand,
                    args,
                    destination,
                    cleanup,
                    ..
                } => {
                    let cleanup = cleanup.clone().map(|block| block.index());
                    let destination = destination
                        .clone()
                        .map(|(place, block)| (place, block.index()));

                    if let mir::Operand::Constant(box func) = func_operand {
                        let func_ty = func.literal.ty();
                        match func_ty.kind() {
                            TyKind::FnDef(def_id, callee_substs) => {
                                ir::TerminatorKind::StaticCall {
                                    callee_did: *def_id,
                                    callee_substs,
                                    args: args.clone(),
                                    cleanup,
                                    destination,
                                }
                            }
                            TyKind::FnPtr(_) => ir::TerminatorKind::FnPtr {
                                value: func.literal.clone(),
                            },
                            _ => panic!("invalid callee of type {:?}", func_ty),
                        }
                    } else {
                        ir::TerminatorKind::Unimplemented("non-constant function call".into())
                    }
                }
                TerminatorKind::Drop { .. } | TerminatorKind::DropAndReplace { .. } => {
                    // TODO: implement Drop and DropAndReplace terminators
                    ir::TerminatorKind::Unimplemented(
                        format!("TODO terminator: {:?}", terminator).into(),
                    )
                }
                _ => ir::TerminatorKind::Unimplemented(
                    format!("Unknown terminator: {:?}", terminator).into(),
                ),
            },
            original: terminator.clone(),
        })
    }

    fn translate_local_decl(&self, local_decl: &mir::LocalDecl<'tcx>) -> ir::LocalDecl<'tcx> {
        ir::LocalDecl { ty: local_decl.ty }
    }

    /// Try to find MIR function body with def_id.
    fn find_fn(
        tcx: TyCtxt<'tcx>,
        def_id: DefId,
    ) -> Result<&'tcx mir::Body<'tcx>, MirInstantiationError> {
        if tcx.is_mir_available(def_id)
            && matches!(
                tcx.hir().body_const_context(def_id.expect_local()),
                None | Some(ConstContext::ConstFn)
            )
        {
            Ok(tcx.optimized_mir(def_id))
        } else {
            debug!(
                "Skipping an item {:?}, no MIR available for this item",
                def_id
            );
            NotAvailable { def_id }.fail()
        }
    }

    pub fn index_adt_cache(&self, adt_did: &DefId) -> Option<&Vec<(LocalDefId, Ty)>> {
        self.adt_impl_cache.get(adt_did)
    }

    pub fn report_level(&self) -> ReportLevel {
        self.report_level
    }
}
