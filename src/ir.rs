//! Reduced MIR intended to cover many common use cases while keeping the analysis pipeline manageable.
//! Use the original MIR definition and support all the features would be ideal, but on the other hand it also would be unrealistic for a research project.
//! We pay the tradeoff up front here, instead of spreading `unimplemented!` all over the place.
use std::borrow::Cow;

use rustc_index::vec::IndexVec;
use rustc_middle::mir;
use rustc_middle::ty::{Instance, Ty};

#[derive(Debug)]
pub struct Terminator<'tcx> {
    pub kind: TerminatorKind<'tcx>,
}

impl<'tcx> Terminator<'tcx> {
    pub fn unimplemented(msg: impl Into<Cow<'static, str>>) -> Self {
        Terminator {
            kind: TerminatorKind::Unimplemented(msg.into()),
        }
    }
}

#[derive(Debug)]
pub enum TerminatorKind<'tcx> {
    Goto(usize),
    Return,
    StaticCall {
        target: Instance<'tcx>,
        args: Vec<mir::Operand<'tcx>>,
        cleanup: Option<usize>,
        destination: (mir::Place<'tcx>, usize),
    },
    Unimplemented(Cow<'static, str>),
    Dummy(&'tcx i32),
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
