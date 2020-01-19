pub mod translate;

use rustc::ty::Ty;

#[derive(Debug)]
pub struct Statement<'tcx> {
    kind: StatementKind<'tcx>,
}

#[derive(Debug)]
pub enum StatementKind<'tcx> {
    Dummy(&'tcx i32),
}

#[derive(Debug)]
pub struct Terminator<'tcx> {
    kind: TerminatorKind<'tcx>,
}

#[derive(Debug)]
pub enum TerminatorKind<'tcx> {
    Dummy(&'tcx i32),
}

#[derive(Debug)]
pub struct BasicBlock<'tcx> {
    statements: Vec<Statement<'tcx>>,
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
