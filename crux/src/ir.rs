use rustc_index::vec::IndexVec;
use rustc_middle::mir;
use rustc_middle::ty::{Instance, Ty};

#[derive(Debug)]
pub struct Terminator<'tcx> {
    pub kind: TerminatorKind<'tcx>,
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
