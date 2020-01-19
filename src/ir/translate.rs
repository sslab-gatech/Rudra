use rustc::mir;
use rustc::ty::{Instance, TyCtxt};

use std::collections::HashMap;

use crate::ext::*;
use crate::ir;

macro_rules! unsupported {
    () => (return Err(TranslateError::Unsupported(String::new())));
    ($($arg:tt)+) => (return Err(TranslateError::Unsupported(format!($($arg)+))));
}

#[derive(Debug, Clone)]
pub enum TranslateError<'tcx> {
    BodyNotAvailable(Instance<'tcx>),
    Unsupported(String),
}

pub type TranslateResult<'tcx, T> = Result<T, TranslateError<'tcx>>;

/// Maps Instance to MIR and cache the result.
pub struct CruxTranslator<'tcx> {
    tcx: TyCtxt<'tcx>,
    cache: HashMap<Instance<'tcx>, TranslateResult<'tcx, ir::Body<'tcx>>>,
}

/// Visit MIR body and returns a Crux IR function
/// Check rustc::mir::visit::Visitor for possible visit targets
/// https://doc.rust-lang.org/nightly/nightly-rustc/rustc/mir/visit/trait.Visitor.html
impl<'tcx> CruxTranslator<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        CruxTranslator {
            tcx,
            cache: HashMap::new(),
        }
    }

    pub fn translate_instance(
        &mut self,
        instance: Instance<'tcx>,
    ) -> TranslateResult<'tcx, &ir::Body<'tcx>> {
        let tcx = self.tcx;
        let result = self.cache.entry(instance).or_insert_with(|| {
            let mir_body = tcx
                .find_fn(instance)
                .body()
                .ok_or_else(|| TranslateError::BodyNotAvailable(instance))?;

            translate_body(mir_body)
        });

        match result {
            Ok(body) => Ok(body),
            Err(e) => Err(e.clone()),
        }
    }
}

fn translate_body<'tcx>(body: &mir::Body<'tcx>) -> TranslateResult<'tcx, ir::Body<'tcx>> {
    let local_decls = body
        .local_decls
        .iter()
        .map(|local_decl| translate_local_decl(local_decl))
        .collect::<Result<Vec<_>, _>>()?;

    let basic_blocks: Vec<_> = body
        .basic_blocks()
        .iter()
        .map(|basic_block| translate_basic_block(basic_block))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ir::Body {
        local_decls,
        basic_blocks,
    })
}

fn translate_basic_block<'tcx>(
    basic_block: &mir::BasicBlockData<'tcx>,
) -> TranslateResult<'tcx, ir::BasicBlock<'tcx>> {
    let statements = basic_block
        .statements
        .iter()
        .map(|statement| statement.clone())
        .collect::<Vec<_>>();

    let terminator = translate_terminator(
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

fn translate_terminator<'tcx>(
    terminator: &mir::Terminator<'tcx>,
) -> TranslateResult<'tcx, ir::Terminator<'tcx>> {
    use mir::TerminatorKind::*;
    Ok(ir::Terminator {
        kind: match terminator.kind {
            Goto { target } => ir::TerminatorKind::Goto(target.index()),
            Call { .. } => todo!("implement call"),
            _ => unsupported!("Unknown terminator: {:?}", terminator),
        },
    })
}

fn translate_local_decl<'tcx>(
    local_decl: &mir::LocalDecl<'tcx>,
) -> TranslateResult<'tcx, ir::LocalDecl<'tcx>> {
    Ok(ir::LocalDecl { ty: local_decl.ty })
}
