use rustc::mir;
use rustc::ty::Ty;

pub mod translate;

#[derive(Debug)]
pub struct Terminator<'tcx> {
    kind: TerminatorKind<'tcx>,
}

#[derive(Debug)]
pub enum TerminatorKind<'tcx> {
    Goto(usize),
    Dummy(&'tcx i32),
}

#[derive(Debug)]
pub struct BasicBlock<'tcx> {
    statements: Vec<mir::Statement<'tcx>>,
    terminator: Terminator<'tcx>,
    is_cleanup: bool,
}

#[derive(Debug)]
pub struct LocalDecl<'tcx> {
    ty: Ty<'tcx>,
}

#[derive(Debug)]
pub struct Body<'tcx> {
    local_decls: Vec<LocalDecl<'tcx>>,
    basic_blocks: Vec<BasicBlock<'tcx>>,
}
