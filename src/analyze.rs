mod error;
mod visitor;

use std::collections::HashMap;

use rustc::mir;
use rustc::ty::{Instance, TyCtxt};

pub use error::{AnalysisError, StepResult};
use visitor::CruxVisitor;

type LocationId = usize;

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

// TODO: implement analysis
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
        let location = self.locations.get_mut(&id).expect("Invalid deallocation");
        *location = LocationContent::Dead;
    }

    pub fn enter_body<'tcx>(&mut self, body: &'tcx mir::Body<'tcx>) -> StepResult<'tcx> {
        if body.arg_count > 0 {
            return Err(AnalysisError::Unsupported(
                "A function with arguments is not supported yet".to_owned(),
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

        let return_id = self.current_stack()[0];
        if self.locations[&return_id].is_valid_return_content(&self.locations) {
            return Err(AnalysisError::InvalidReturnContent);
        }

        self.stack_frame.pop();

        Ok(())
    }

    fn current_stack(&mut self) -> &mut Vec<LocationId> {
        self.stack_frame.last_mut().expect("Stack underflow")
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
