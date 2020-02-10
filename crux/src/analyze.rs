mod simple_anderson;
pub mod solver;

use rustc::ty::Ty;

pub use simple_anderson::SimpleAnderson;

type NodeId = usize;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Location<'tcx> {
    id: usize,
    /// `None` for temporary variables introduced during lowering process
    ty: Option<Ty<'tcx>>,
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

    fn next(&mut self, ty: Option<Ty<'tcx>>) -> Location<'tcx> {
        let counter = self.counter;
        self.counter
            .checked_add(1)
            .expect("location counter overflow");
        Location { id: counter, ty }
    }

    fn num_locations(&self) -> usize {
        self.counter
    }

    fn clear(&mut self) {
        self.counter = 0;
        self.list.clear();
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Constraint {
    /// A >= {B}
    AddrOf(NodeId),
    /// A >= B
    Copy(NodeId),
    /// A >= *B
    Load(NodeId),
    /// *A >= B
    Store(NodeId),
    /// *A >= {B}
    StoreAddr(NodeId),
}

pub trait ConstraintSet {
    type Iter: Iterator<Item = (NodeId, Constraint)>;

    fn num_locations(&self) -> usize;
    fn constraints(&self) -> Self::Iter;
}
