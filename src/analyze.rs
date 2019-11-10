mod error;
mod visitor;

use std::collections::HashMap;

use rustc::mir;
use rustc::ty::{Instance, TyCtxt};

pub use error::{AnalysisError, StepResult};
use visitor::CruxVisitor;

type LocationId = usize;

#[derive(Clone, Debug)]
pub enum LocationContent {
    Dead,
    Uninitialized,
    Value,
    // possible set of locations
    Locations(Vec<LocationId>),
}

impl LocationContent {
    pub fn is_valid_return_content(
        &self,
        location_map: &HashMap<LocationId, LocationContent>,
    ) -> bool {
        match self {
            LocationContent::Dead | LocationContent::Uninitialized => false,
            LocationContent::Value => true,
            LocationContent::Locations(ref locations) => {
                // TODO: recursive shape analysis?
                locations
                    .iter()
                    .all(|location| match location_map.get(location) {
                        None => false,
                        Some(LocationContent::Dead) => false,
                        _ => true,
                    })
            }
        }
    }
}

pub struct Analyzer<'tcx> {
    tcx: TyCtxt<'tcx>,
}

// TODO: implement analysis summary
pub struct AnalysisSummary {}

impl AnalysisSummary {
    pub fn new() -> Self {
        AnalysisSummary {}
    }
}

pub struct AnalysisContext {
    id_counter: LocationId,
    locations: HashMap<LocationId, LocationContent>,
    stack_frame: Vec<Vec<LocationId>>,
}

impl AnalysisContext {
    pub fn new() -> Self {
        AnalysisContext {
            id_counter: 0,
            locations: HashMap::new(),
            stack_frame: Vec::new(),
        }
    }

    pub fn generate_summary(&self) -> AnalysisSummary {
        AnalysisSummary::new()
    }

    fn allocate(&mut self) -> LocationId {
        let result = self.id_counter;
        self.id_counter = self
            .id_counter
            .checked_add(1)
            .expect("Location ID overflowed");
        assert!(self
            .locations
            .insert(result, LocationContent::Uninitialized)
            .is_none());

        result
    }

    fn deallocate(&mut self, id: LocationId) {
        let location = self.lookup(id);
        *location = LocationContent::Dead;
    }

    pub fn enter_body<'tcx>(&mut self, body: &'tcx mir::Body<'tcx>) -> StepResult<'tcx> {
        if body.arg_count > 0 {
            return Err(AnalysisError::Unimplemented(
                "Function arguments are not supported yet".to_owned(),
                Some(body.span),
            ));
        }

        let mut stack = Vec::new();
        for _local_decl in body.local_decls.iter() {
            // TODO: handle arguments
            stack.push(self.allocate());
        }
        self.stack_frame.push(stack);

        Ok(())
    }

    pub fn exit_body<'tcx>(&mut self, body: &'tcx mir::Body<'tcx>) -> StepResult<'tcx> {
        for (idx, _local_decl) in body.local_decls.iter().enumerate() {
            if idx == 0 {
                // return value is not deallocated here
                // TODO: deallocate on caller side
            } else if idx <= body.arg_count {
                unimplemented!()
            } else {
                let local_id = self.current_stack()[idx];
                self.deallocate(local_id);
            }
        }

        dbg!(&self.stack_frame);
        dbg!(&self.locations);

        let return_id = self.current_stack()[0];
        if !self.locations[&return_id].is_valid_return_content(&self.locations) {
            return Err(AnalysisError::InvalidReturnContent);
        }

        self.stack_frame.pop();

        Ok(())
    }

    fn current_stack(&self) -> &Vec<LocationId> {
        self.stack_frame.last().expect("Stack underflow")
    }

    pub fn lookup(&mut self, id: LocationId) -> &mut LocationContent {
        self.locations
            .get_mut(&id)
            .expect("Invalid lookup location")
    }

    /// This function handles the update of the location.
    /// If source and the destinations are both location sets,
    /// it merges two sets based on points-to analysis.
    /// Otherwise, it overwrites the destination.
    /// Note that this function doesn't handle an update to the uninitialized value,
    /// which happens in move semantics.
    pub fn update_location<'tcx>(
        &mut self,
        id: LocationId,
        content: LocationContent,
    ) -> StepResult<'tcx> {
        let location_ptr = self.lookup(id);

        match (location_ptr, content) {
            (LocationContent::Dead, _) => return Err(AnalysisError::WriteToDeadLocation),
            (location_ptr @ LocationContent::Uninitialized, content) => *location_ptr = content,
            (LocationContent::Value, LocationContent::Value) => (),
            (
                LocationContent::Locations(ref mut dst_locations),
                LocationContent::Locations(ref src_locations),
            ) => {
                // TODO: use better points-to analysis such as Steensgard's algorithm
                for location in src_locations.iter() {
                    if !dst_locations.contains(location) {
                        dst_locations.push(*location);
                    }
                }
            }
            (location_ptr, content @ LocationContent::Dead) => *location_ptr = content,
            (location_ptr, content) => {
                return Err(AnalysisError::Unimplemented(
                    format!(
                        "Unexpected merge between `{:?}` and `{:?}`",
                        location_ptr, content
                    ),
                    None,
                ))
            }
        }

        Ok(())
    }

    pub fn resolve_place<'tcx>(
        &self,
        place: &mir::Place<'tcx>,
    ) -> Result<LocationId, AnalysisError<'tcx>> {
        let mut current = match place.base {
            mir::PlaceBase::Local(local) => self.current_stack()[local.as_usize()],
            mir::PlaceBase::Static(_) => {
                return Err(AnalysisError::Unimplemented(
                    format!("Static place base is not supported: {:?}", place),
                    None,
                ))
            }
        };

        for projection_elem in place.projection.into_iter() {
            use mir::ProjectionElem::*;
            match projection_elem {
                Deref => match self.locations.get(&current) {
                    Some(LocationContent::Locations(ref location_vec)) => {
                        if location_vec.len() != 1 {
                            return Err(AnalysisError::Unimplemented(
                                "Deref target may contain multiple locations".to_owned(),
                                None,
                            ));
                        }
                        current = location_vec[0];
                    }
                    _ => {
                        return Err(AnalysisError::Unimplemented(
                            "Deref projection is only supported on pointer types".to_owned(),
                            None,
                        ))
                    }
                },
                _ => {
                    return Err(AnalysisError::Unimplemented(
                        format!("Unsupported place projection: {:?}", place),
                        None,
                    ))
                }
            }
        }

        Ok(current)
    }

    pub fn handle_assign<'tcx>(
        &mut self,
        dst: &mir::Place<'tcx>,
        src: &mir::Operand<'tcx>,
    ) -> StepResult<'tcx> {
        let dst_id = self.resolve_place(dst)?;
        match src {
            mir::Operand::Copy(src) => {
                let src_id = self.resolve_place(src)?;
                let content = self.lookup(src_id).clone();
                self.update_location(dst_id, content)?;
            }
            mir::Operand::Move(src) => {
                let src_id = self.resolve_place(src)?;
                let content = self
                    .locations
                    .insert(src_id, LocationContent::Uninitialized)
                    .expect("Invalid move source");
                self.update_location(dst_id, content)?;
            }
            mir::Operand::Constant(_) => self.update_location(dst_id, LocationContent::Value)?,
        }
        Ok(())
    }

    pub fn handle_ref<'tcx>(
        &mut self,
        dst: &mir::Place<'tcx>,
        src: &mir::Place<'tcx>,
    ) -> StepResult<'tcx> {
        let dst_id = self.resolve_place(dst)?;
        let src_id = self.resolve_place(src)?;
        self.update_location(dst_id, LocationContent::Locations(vec![src_id]))
    }
}

impl<'tcx> Analyzer<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Analyzer { tcx }
    }

    pub fn analyze(
        &mut self,
        entry: Instance<'tcx>,
    ) -> Result<AnalysisSummary, AnalysisError<'tcx>> {
        let mut acx = AnalysisContext::new();
        let mut visitor = CruxVisitor::new();
        visitor.visit_instance(self.tcx, &mut acx, entry)?;
        Ok(acx.generate_summary())
    }
}
