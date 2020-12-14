use std::rc::Rc;

use rustc_hir::{BodyId, HirId};
use rustc_middle::mir;
use rustc_middle::ty::{self, Instance, InstanceDef, TyCtxt, TyKind};

use dashmap::DashMap;
use snafu::Snafu;

use crate::ir;
use crate::prelude::*;
use crate::visitor::{RelatedFnCollector, RelatedItemMap};

#[derive(Debug, Snafu, Clone)]
pub enum MirInstantiationError<'tcx> {
    Foreign {
        instance: Instance<'tcx>,
    },
    Virtual {
        instance: Instance<'tcx>,
    },
    NotAvailable {
        instance: Instance<'tcx>,
    },
    UnknownDef {
        instance: Instance<'tcx>,
        def: InstanceDef<'tcx>,
    },
}

pub type RudraCtxt<'tcx> = &'tcx RudraCtxtOwner<'tcx>;
pub type TranslationResult<'tcx, T> = Result<T, MirInstantiationError<'tcx>>;

/// Maps Instance to MIR and cache the result.
pub struct RudraCtxtOwner<'tcx> {
    tcx: TyCtxt<'tcx>,
    translation_cache: DashMap<Instance<'tcx>, Rc<TranslationResult<'tcx, ir::Body<'tcx>>>>,
    related_item_cache: RelatedItemMap,
}

/// Visit MIR body and returns a Rudra IR function
/// Check rustc::mir::visit::Visitor for possible visit targets
/// https://doc.rust-lang.org/nightly/nightly-rustc/rustc/mir/visit/trait.Visitor.html
impl<'tcx> RudraCtxtOwner<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        RudraCtxtOwner {
            tcx,
            translation_cache: DashMap::new(),
            related_item_cache: RelatedFnCollector::collect(tcx),
        }
    }

    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn related_items(&self, type_hir_id: HirId) -> Option<&Vec<BodyId>> {
        self.related_item_cache.get(&type_hir_id)
    }

    pub fn types_with_related_items(&self) -> impl Iterator<Item = (HirId, BodyId)> + '_ {
        (&self.related_item_cache)
            .into_iter()
            .flat_map(|(&k, v)| v.iter().map(move |&body_id| (k, body_id)))
    }

    pub fn instance_body(
        &self,
        instance: Instance<'tcx>,
    ) -> Rc<TranslationResult<'tcx, ir::Body<'tcx>>> {
        let tcx = self.tcx();
        let result = self.translation_cache.entry(instance).or_insert_with(|| {
            Rc::new(
                try {
                    let mir_body = Self::find_fn(tcx, instance)?;
                    self.translate_body(instance, mir_body)?
                },
            )
        });

        result.clone()
    }

    fn translate_body(
        &self,
        instance: Instance<'tcx>,
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
            .map(|basic_block| self.translate_basic_block(instance, basic_block))
            .collect::<Result<Vec<_>, _>>()?;

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
    ) -> TranslationResult<'tcx, ir::BasicBlock<'tcx>> {
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
    ) -> TranslationResult<'tcx, ir::Terminator<'tcx>> {
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
                            return Ok(ir::Terminator::unimplemented(
                                "function call does not return",
                            ));
                        }
                    };

                    if let mir::Operand::Constant(box func) = func_operand {
                        let func_ty = func.literal.ty;
                        match func_ty.kind {
                            TyKind::FnDef(def_id, callee_substs) => {
                                let instance = self
                                    .tcx()
                                    .ext()
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
                                ir::TerminatorKind::Unimplemented("function pointer".into())
                            }
                            _ => panic!("invalid callee of type {:?}", func_ty),
                        }
                    } else {
                        ir::TerminatorKind::Unimplemented("non-constant function call".into())
                    }
                }
                _ => ir::TerminatorKind::Unimplemented(
                    format!("Unknown terminator: {:?}", terminator).into(),
                ),
            },
        })
    }

    fn translate_local_decl(&self, local_decl: &mir::LocalDecl<'tcx>) -> ir::LocalDecl<'tcx> {
        ir::LocalDecl { ty: local_decl.ty }
    }

    /// Try to find MIR function body with given Instance
    /// this is a combined version of MIRI's find_fn + Rust InterpCx's load_mir
    fn find_fn(
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
    ) -> Result<&'tcx mir::Body<'tcx>, MirInstantiationError<'tcx>> {
        // TODO: apply hooks in rustc MIR evaluator based on this
        // https://github.com/rust-lang/miri/blob/1037f69bf6dcf73dfbe06453336eeae61ba7c51f/src/shims/mod.rs

        // currently we don't handle any foreign item
        if tcx.is_foreign_item(instance.def_id()) {
            return Foreign { instance }.fail();
        }

        // based on rustc InterpCx's `load_mir()`
        // https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir/interpret/eval_context.rs.html
        let def_id = instance.def.with_opt_param();
        if let Some(def) = def_id.as_local() {
            if tcx.has_typeck_results(def.did) {
                if let Some(_) = tcx.typeck_opt_const_arg(def).tainted_by_errors {
                    // type check failure; shouldn't happen since we already ran `cargo check`
                    panic!("Type check failed for an item: {:?}", &instance);
                }
            }
        }

        match instance.def {
            ty::InstanceDef::Item(def) => {
                if tcx.is_mir_available(def.did) {
                    if let Some((did, param_did)) = def.as_const_arg() {
                        Ok(tcx.optimized_mir_of_const_arg((did, param_did)))
                    } else {
                        Ok(tcx.optimized_mir(def.did))
                    }
                } else {
                    debug!(
                        "Skipping an item {:?}, no MIR available for this item",
                        &instance
                    );
                    NotAvailable { instance }.fail()
                }
            }
            ty::InstanceDef::DropGlue(_, _) => Ok(tcx.instance_mir(instance.def)),
            ty::InstanceDef::Virtual(_, _) => Virtual { instance }.fail(),
            _ => UnknownDef {
                instance,
                def: instance.def,
            }
            .fail(),
        }
    }
}
