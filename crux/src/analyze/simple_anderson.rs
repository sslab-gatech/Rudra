//! Flow-insensitive, context-insensitive, and field-insensitive
//! variants of Andersen's points-to analysis
//! based on the paper "Field-sensitive pointer analysis for C"
use std::collections::HashMap;
use std::collections::HashSet;

use rustc::mir;
use rustc::ty::{Instance, Ty};

use super::{Constraint, ConstraintSet, Location, LocationFactory};
use crate::error::{Error, Result};
use crate::prelude::*;

macro_rules! unimplemented {
    () => (return Err(Error::AnalysisUnimplemented(String::new())));
    ($($arg:tt)+) => (return Err(Error::AnalysisUnimplemented(format!($($arg)+))));
}

#[derive(Clone, Debug)]
struct Place<'tcx> {
    base: Location<'tcx>,
    deref_count: usize,
}

// TODO: add translation cache here
pub struct SimpleAnderson<'ccx, 'tcx> {
    ccx: CruxCtxt<'ccx, 'tcx>,
    location_factory: LocationFactory<'tcx>,
    /// Analysis call stack
    call_stack: Vec<Instance<'tcx>>,
    /// Collection of constraints
    constraints: Vec<HashSet<Constraint>>,
    local_var_map: HashMap<Instance<'tcx>, Vec<Location<'tcx>>>,
}

impl<'ccx, 'tcx> ConstraintSet for SimpleAnderson<'ccx, 'tcx> {
    type Iter = std::vec::IntoIter<(usize, Constraint)>;

    fn num_locations(&self) -> usize {
        self.location_factory.num_locations()
    }

    fn constraints(&self) -> Self::Iter {
        let mut vec = Vec::new();

        for location_id in 0..self.num_locations() {
            let constraint_pair_iter = self.constraints[location_id]
                .iter()
                .map(|constraint| (location_id, constraint.clone()));
            vec.extend(constraint_pair_iter);
        }

        vec.into_iter()
    }
}

impl<'ccx, 'tcx> SimpleAnderson<'ccx, 'tcx> {
    pub fn new(ccx: CruxCtxt<'ccx, 'tcx>) -> Self {
        SimpleAnderson {
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
                mir::ProjectionElem::Deref => count += 1,
                _ => unimplemented!("Projection: {:?}", projection),
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

    fn gen_location(&mut self, ty: Option<Ty<'tcx>>) -> Location<'tcx> {
        self.constraints.push(HashSet::new());
        self.location_factory.next(ty)
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

    fn add_constraint(&mut self, dst_id: usize, constraint: Constraint) {
        self.constraints[dst_id].insert(constraint);
    }

    /// The main entry point of the analysis
    pub fn analyze(&mut self, instance: Instance<'tcx>) -> Result<'tcx, ()> {
        self.clear();
        self.visit_body(instance)?;

        todo!("check constraints")
    }

    fn visit_body(&mut self, instance: Instance<'tcx>) -> Result<'tcx, ()> {
        if self.analyzed(instance) {
            return Ok(());
        }

        // find MIR for the instance
        let body = self.ccx.instance_body(instance);
        let body = match &*body {
            Ok(body) => body,
            Err(e) => return Err(e.clone()),
        };

        // instantiate local variables
        let locations = body
            .local_decls
            .iter()
            .map(|local_decl| self.gen_location(Some(local_decl.ty)))
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
                            rvalue => unimplemented!("Rvalue `{:?}`", rvalue),
                        }
                    }

                    // NOP
                    StorageLive(_) | StorageDead(_) | Nop => (),

                    // TODO: support more statements
                    _ => unimplemented!("Statement `{:?}`", statement),
                }
            }

            // TODO: handle terminator
        }

        self.call_stack.pop();

        Ok(())
    }

    fn handle_assign<T>(
        &mut self,
        local_decls: &T,
        dst: &mir::Place<'tcx>,
        src: &mir::Operand<'tcx>,
    ) -> Result<'tcx, ()>
    where
        T: mir::HasLocalDecls<'tcx>,
    {
        let src_is_ptr = self.is_operand_ptr(local_decls, src);
        let dst_is_ptr = self.is_place_ptr(local_decls, dst);

        if src_is_ptr && dst_is_ptr {
            match src {
                mir::Operand::Copy(src) | mir::Operand::Move(src) => {
                    let dst = self.lower_mir_place(dst)?;
                    let src = self.lower_mir_place(src)?;

                    self.handle_place_to_place(dst, src)?;
                }
                mir::Operand::Constant(_) => unimplemented!("Constant pointer: {:?}", src),
            }
        } else if dst_is_ptr && !src_is_ptr {
            unimplemented!("Cast to pointer: from `{:?}` to `{:?}`", src, dst);
        }

        Ok(())
    }

    fn handle_ref<T>(
        &mut self,
        local_decls: &T,
        dst: &mir::Place<'tcx>,
        src: &mir::Place<'tcx>,
    ) -> Result<'tcx, ()>
    where
        T: mir::HasLocalDecls<'tcx>,
    {
        let dst_is_ptr = self.is_place_ptr(local_decls, dst);
        assert!(dst_is_ptr);

        let dst = self.lower_mir_place(dst)?;
        let src = self.lower_mir_place(src)?;

        if src.deref_count == 0 {
            if dst.deref_count == 0 {
                self.add_constraint(dst.base.id, Constraint::AddrOf(src.base.id))
            } else {
                let mut current_dst = dst;
                while current_dst.deref_count > 1 {
                    let next_base = self.gen_location(None);
                    self.add_constraint(next_base.id, Constraint::Load(current_dst.base.id));
                    current_dst = Place {
                        base: next_base,
                        deref_count: current_dst.deref_count - 1,
                    };
                }
                self.add_constraint(current_dst.base.id, Constraint::StoreAddr(src.base.id))
            }
        } else {
            // Replace &* pattern
            let Place {
                base: src_base,
                deref_count: src_deref_count,
            } = src;
            self.handle_place_to_place(
                dst,
                Place {
                    base: src_base,
                    deref_count: src_deref_count - 1,
                },
            )?;
        }

        Ok(())
    }

    fn handle_place_to_place(&mut self, dst: Place<'tcx>, src: Place<'tcx>) -> Result<'tcx, ()> {
        match (dst.deref_count, src.deref_count) {
            (0, 0) => self.add_constraint(dst.base.id, Constraint::Copy(src.base.id)),
            (0, _) => {
                let mut current_src = src;
                while current_src.deref_count > 1 {
                    let next_base = self.gen_location(None);
                    self.add_constraint(next_base.id, Constraint::Load(current_src.base.id));
                    current_src = Place {
                        base: next_base,
                        deref_count: current_src.deref_count - 1,
                    };
                }
                self.add_constraint(dst.base.id, Constraint::Load(current_src.base.id))
            }
            (_, _) => {
                let mut current_src = src;
                while current_src.deref_count >= 1 {
                    let next_base = self.gen_location(None);
                    self.add_constraint(next_base.id, Constraint::Load(current_src.base.id));
                    current_src = Place {
                        base: next_base,
                        deref_count: current_src.deref_count - 1,
                    };
                }
                let mut current_dst = dst;
                while current_dst.deref_count > 1 {
                    let next_base = self.gen_location(None);
                    self.add_constraint(next_base.id, Constraint::Load(current_dst.base.id));
                    current_dst = Place {
                        base: next_base,
                        deref_count: current_dst.deref_count - 1,
                    };
                }
                self.add_constraint(current_dst.base.id, Constraint::Store(current_src.base.id))
            }
        }
        Ok(())
    }
}
