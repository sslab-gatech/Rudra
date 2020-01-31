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

#[derive(Clone, Copy, Debug, PartialEq)]
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
struct Place<'tcx> {
    base: Location<'tcx>,
    deref_count: usize,
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

    fn local_to_location(&self, local: mir::Local) -> Location<'tcx> {
        self.local_var_map[self.call_stack.last().unwrap()][local.index()]
    }

    fn lower_mir_place(
        &self,
        place: &mir::Place<'tcx>,
    ) -> std::result::Result<Place<'tcx>, Error<'tcx>> {
        let base = self.local_to_location(place.local);

        let mut count = 0;
        for projection in place.projection {
            match projection {
                Deref => count += 1,
                _ => {
                    return Err(Error::Unimplemented(format!(
                        "Projection: {:?}",
                        projection
                    )))
                }
            }
        }

        Ok(Place {
            base,
            deref_count: count,
        })
    }

    fn analyzed(&self, instance: Instance<'tcx>) -> bool {
        self.local_var_map.contains_key(&instance)
    }

    fn is_operand_ptr(
        &self,
        local_decls: &impl mir::HasLocalDecls<'tcx>,
        operand: &mir::Operand<'tcx>,
    ) -> bool {
        operand.ty(local_decls, self.ccx.tcx()).is_any_ptr()
    }

    fn is_place_ptr(
        &self,
        local_decls: &impl mir::HasLocalDecls<'tcx>,
        place: &mir::Place<'tcx>,
    ) -> bool {
        place.ty(local_decls, self.ccx.tcx()).ty.is_any_ptr()
    }

    fn add_constraint(&mut self, from: usize, edge: Constraint) {
        todo!()
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
        for basic_block in body.basic_blocks.iter() {
            for statement in basic_block.statements.iter() {
                use mir::StatementKind::*;
                match statement.kind {
                    Assign(box (ref dst, ref rvalue)) => {
                        use mir::Rvalue::*;
                        match rvalue {
                            Use(operand) => self.handle_assign(body, dst, operand)?,
                            Cast(_, ref operand, _) => self.handle_assign(body, dst, operand)?,

                            AddressOf(_, ref src) => self.handle_ref(body, dst, src)?,
                            Ref(_, _, ref src) => self.handle_ref(body, dst, src)?,

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

        self.call_stack.pop();

        Ok(())
    }

    fn handle_assign<T>(
        &mut self,
        local_decls: &T,
        dst: &mir::Place<'tcx>,
        src: &mir::Operand<'tcx>,
    ) -> Result<'tcx>
    where
        T: mir::HasLocalDecls<'tcx>,
    {
        let src_is_ptr = self.is_operand_ptr(local_decls, src);
        let dst_is_ptr = self.is_place_ptr(local_decls, dst);

        if src_is_ptr && dst_is_ptr {
            match src {
                mir::Operand::Copy(src) | mir::Operand::Move(src) => {
                    let src = self.lower_mir_place(src)?;
                    let dst = self.lower_mir_place(dst)?;

                    todo!()
                }
                mir::Operand::Constant(_) => {
                    return Err(Error::Unimplemented(format!("Constant pointer: {:?}", src)));
                }
            }
        } else if dst_is_ptr && !src_is_ptr {
            return Err(Error::Unimplemented(format!(
                "Cast to pointer: from `{:?}` to `{:?}`",
                src, dst
            )));
        }

        Ok(())
    }

    fn handle_ref<T>(
        &mut self,
        local_decls: &T,
        dst: &mir::Place<'tcx>,
        src: &mir::Place<'tcx>,
    ) -> Result<'tcx>
    where
        T: mir::HasLocalDecls<'tcx>,
    {
        let dst_is_ptr = self.is_place_ptr(local_decls, dst);
        assert!(dst_is_ptr);

        let src = self.lower_mir_place(src)?;
        let dst = self.lower_mir_place(dst)?;

        todo!()
    }
}
