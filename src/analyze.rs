//! Anderson's points-to analysis
mod error;
mod graph;

use rustc::mir;
use rustc::ty::{Instance, Ty};

pub use self::error::{Error, Result};
use self::graph::Scc;
use crate::ir;
pub use crate::prelude::*;

pub struct Analyzer<'ccx, 'tcx> {
    ccx: CruxCtxt<'ccx, 'tcx>,
    session: Session<'tcx>,
}

struct LocationGenerator {
    counter: usize,
}

impl LocationGenerator {
    fn new() -> Self {
        LocationGenerator { counter: 0 }
    }

    fn next<'tcx>(&mut self, ty: Ty<'tcx>) -> Location<'tcx> {
        let counter = self.counter;
        self.counter
            .checked_add(1)
            .expect("location counter overflow");
        Location { id: counter, ty }
    }
}

pub struct Location<'tcx> {
    id: usize,
    ty: Ty<'tcx>,
}

pub struct CallingContext<'tcx> {
    instance: Instance<'tcx>,
    locations: Vec<Location<'tcx>>,
}

pub struct Session<'tcx> {
    location_generator: LocationGenerator,
    calling_context: Vec<CallingContext<'tcx>>,
}

impl<'tcx> Session<'tcx> {
    fn empty() -> Self {
        Session {
            location_generator: LocationGenerator::new(),
            calling_context: Vec::new(),
        }
    }
}

impl<'ccx, 'tcx> Analyzer<'ccx, 'tcx> {
    pub fn new(ccx: CruxCtxt<'ccx, 'tcx>) -> Self {
        Analyzer {
            ccx,
            session: Session::empty(),
        }
    }

    pub fn enter(&mut self, instance: Instance<'tcx>) -> Result<'tcx> {
        // TODO: handle functions with arguments
        self.session = Session::empty();

        self.visit_body(instance)
    }

    fn visit_body(&mut self, instance: Instance<'tcx>) -> Result<'tcx> {
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
            .map(|local_decl| self.session.location_generator.next(local_decl.ty))
            .collect::<Vec<_>>();

        self.session.calling_context.push(CallingContext {
            instance,
            locations,
        });

        // traverse
        let scc = Scc::construct(body);
        let group_order = scc.topological_order();

        for &group in group_order.iter() {
            self.visit_group(&scc, group)?;
        }

        todo!()
    }

    fn visit_group(&mut self, scc: &Scc<ir::Body<'tcx>>, group: usize) -> Result<'tcx> {
        // TODO: calculate fixed point
        for &basic_block in scc.nodes_in_group(group) {
            self.visit_basic_block(&scc.graph().basic_blocks[basic_block])?;
        }

        Ok(())
    }

    fn visit_basic_block(&mut self, basic_block: &ir::BasicBlock<'tcx>) -> Result<'tcx> {
        for statement in basic_block.statements.iter() {
            self.visit_statement(statement)?;
        }

        Ok(())
    }

    fn visit_statement(&mut self, _statement: &mir::Statement<'tcx>) -> Result<'tcx> {
        todo!()
    }
}
