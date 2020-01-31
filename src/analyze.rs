//! Flow-insensitive, context-insensitive, and field-insensitive
//! variants of Andersen's points-to analysis
//! based on the paper "Field-sensitive pointer analysis for C"
mod error;

use std::collections::HashMap;
use std::collections::HashSet;

use rustc::mir;
use rustc::ty::{Instance, Ty};

pub use self::error::{Error, Result};
pub use crate::prelude::*;

// TODO: add translation cache here
pub struct Analyzer<'ccx, 'tcx> {
    ccx: CruxCtxt<'ccx, 'tcx>,
    location_factory: LocationFactory<'tcx>,
    /// Analysis call stack
    call_stack: Vec<Instance<'tcx>>,
    /// Collection of constraints
    constraints: Vec<HashSet<Constraint>>,
    local_var_map: HashMap<Instance<'tcx>, Vec<Location<'tcx>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Location<'tcx> {
    id: usize,
    ty: Ty<'tcx>,
}

struct LocationFactory<'tcx> {
    counter: usize,
    list: Vec<Location<'tcx>>,
}

impl<'tcx> LocationFactory<'tcx> {
    fn new() -> Self {
        LocationFactory {
            counter: 0,
            list: Vec::new(),
        }
    }

    fn next(&mut self, ty: Ty<'tcx>) -> Location<'tcx> {
        let counter = self.counter;
        self.counter
            .checked_add(1)
            .expect("location counter overflow");
        Location { id: counter, ty }
    }

    fn clear(&mut self) {
        self.counter = 0;
        self.list.clear();
    }
}

#[derive(Clone, Debug)]
struct Constraint {
    to: usize,
    kind: ConstraintKind,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum ConstraintKind {
    /// A >= {B}
    AddrOf(usize),
    /// A >= B
    Copy(usize),
    /// A >= *B
    Load(usize),
    /// *A >= B
    Store(usize),
}

impl<'ccx, 'tcx> Analyzer<'ccx, 'tcx> {
    pub fn new(ccx: CruxCtxt<'ccx, 'tcx>) -> Self {
        Analyzer {
            ccx,
            location_factory: LocationFactory::new(),
            call_stack: Vec::new(),
            constraints: Vec::new(),
            local_var_map: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.location_factory.clear();
        self.call_stack.clear();
        self.constraints.clear();
    }

    fn add_constraint(&mut self, from: usize, edge: Constraint) {
        todo!()
    }

    fn analyzed(&self, instance: Instance<'tcx>) -> bool {
        self.local_var_map.contains_key(&instance)
    }

    pub fn enter(&mut self, instance: Instance<'tcx>) -> Result<'tcx> {
        self.clear();
        self.visit_body(instance)?;

        todo!("check constraints")
    }

    fn visit_body(&mut self, instance: Instance<'tcx>) -> Result<'tcx> {
        if self.analyzed(instance) {
            return Ok(());
        }

        // find MIR for the instance
        let body = self.ccx.instance_body(instance);
        let body = match &*body {
            Ok(body) => body,
            Err(_) => return Err(Error::BodyNotAvailable(instance)),
        };

        // instantiate local variables
        let locations = body
            .local_decls
            .iter()
            .map(|local_decl| self.location_factory.next(local_decl.ty))
            .collect::<Vec<_>>();

        // mark the function as visited
        self.local_var_map.insert(instance, locations);
        self.call_stack.push(instance);

        // we are implementing a flow-insensitive analysis,
        // so the visiting order doesn't matter
        let tcx = self.ccx.tcx();
        for basic_block in body.basic_blocks.iter() {
            for statement in basic_block.statements.iter() {
                use mir::StatementKind::*;
                match statement.kind {
                    Assign(box (ref dst, ref rvalue)) => {
                        use mir::Rvalue::*;
                        match rvalue {
                            Use(operand) => self.handle_assign(dst, operand)?,

                            Cast(_, ref operand, _) => {
                                let src_is_ptr = operand.ty(body, tcx).is_any_ptr();
                                let dst_is_ptr = dst.ty(body, tcx).ty.is_any_ptr();
                                if dst_is_ptr {
                                    if src_is_ptr {
                                        self.handle_assign(dst, operand)?;
                                    } else {
                                        return Err(Error::Unimplemented(format!(
                                            "Pointer casting is not supported `{:?}`",
                                            statement
                                        )));
                                    }
                                }
                            }

                            AddressOf(_, ref src) => self.handle_ref(dst, src)?,
                            Ref(_, _, ref src) => self.handle_ref(dst, src)?,

                            BinaryOp(_, _, _) | CheckedBinaryOp(_, _, _) | UnaryOp(_, _) => (),

                            // TODO: support more rvalue
                            rvalue => {
                                return Err(Error::Unimplemented(format!("Rvalue `{:?}`", rvalue)))
                            }
                        }
                    }

                    // NOP
                    StorageLive(_) | StorageDead(_) | Nop => (),

                    // TODO: support more statements
                    _ => return Err(Error::Unimplemented(format!("Statement `{:?}`", statement))),
                }
            }
        }

        Ok(())
    }

    fn handle_assign(
        &mut self,
        dst: &mir::Place<'tcx>,
        operand: &mir::Operand<'tcx>,
    ) -> Result<'tcx> {
        todo!()
    }

    fn handle_ref(&mut self, dst: &mir::Place<'tcx>, src: &mir::Place<'tcx>) -> Result<'tcx> {
        todo!()
    }
}
