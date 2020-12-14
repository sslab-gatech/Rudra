//! Reduced MIR intended to cover many common use cases while keeping the analysis pipeline manageable.
//! Note that this is a translation of non-monomorphized, generic MIR.

use std::borrow::Cow;

use rustc_hir::def_id::DefId;
use rustc_index::vec::IndexVec;
use rustc_middle::{
    mir,
    ty::{self, Ty},
};

#[derive(Debug)]
pub struct Terminator<'tcx> {
    pub kind: TerminatorKind<'tcx>,
    pub original: mir::Terminator<'tcx>,
}

#[derive(Debug)]
pub enum TerminatorKind<'tcx> {
    Goto(usize),
    Return,
    StaticCall {
        callee_did: DefId,
        args: Vec<mir::Operand<'tcx>>,
        cleanup: Option<usize>,
        destination: Option<(mir::Place<'tcx>, usize)>,
    },
    FnPtr {
        value: ty::ConstKind<'tcx>,
    },
    Unimplemented(Cow<'static, str>),
}

#[derive(Debug)]
pub struct BasicBlock<'tcx> {
    pub statements: Vec<mir::Statement<'tcx>>,
    pub terminator: Terminator<'tcx>,
    pub is_cleanup: bool,
}

#[derive(Debug)]
pub struct LocalDecl<'tcx> {
    pub ty: Ty<'tcx>,
}

#[derive(Debug)]
pub struct Body<'tcx> {
    pub local_decls: Vec<LocalDecl<'tcx>>,
    pub original_decls: IndexVec<mir::Local, mir::LocalDecl<'tcx>>,
    pub basic_blocks: Vec<BasicBlock<'tcx>>,
}

impl<'tcx> mir::HasLocalDecls<'tcx> for Body<'tcx> {
    fn local_decls(&self) -> &IndexVec<mir::Local, mir::LocalDecl<'tcx>> {
        &self.original_decls
    }
}

impl<'tcx> Body<'tcx> {
    pub fn terminators(&self) -> impl Iterator<Item = &Terminator<'tcx>> {
        self.basic_blocks.iter().map(|block| &block.terminator)
    }
}
