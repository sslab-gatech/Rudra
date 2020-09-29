mod call_graph;
mod simple_anderson;
pub mod solver;
mod unsafe_destructor;

use rustc_middle::ty::Ty;

use snafu::{Error, ErrorCompat};

pub use call_graph::CallGraph;
pub use simple_anderson::SimpleAnderson;
pub use unsafe_destructor::UnsafeDestructor;

pub type AnalysisResult<'tcx, T> = Result<T, Box<dyn AnalysisError + 'tcx>>;

pub trait AnalysisError: Error + ErrorCompat {
    fn kind(&self) -> AnalysisErrorKind;
    fn log(&self) {
        match self.kind() {
            AnalysisErrorKind::Unreachable => {
                error!("[{:?}] {}", self.kind(), self);
                if let Some(backtrace) = ErrorCompat::backtrace(self) {
                    error!("{}", backtrace);
                }
            }
            AnalysisErrorKind::Unimplemented => {
                info!("[{:?}] {}", self.kind(), self);
                if let Some(backtrace) = ErrorCompat::backtrace(self) {
                    info!("{}", backtrace);
                }
            }
            AnalysisErrorKind::OutOfScope => {
                debug!("[{:?}] {}", self.kind(), self);
                if let Some(backtrace) = ErrorCompat::backtrace(self) {
                    debug!("{}", backtrace);
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum AnalysisErrorKind {
    /// An error that should never happen;
    /// If this happens, that means some of our assumption / invariant is broken.
    /// Normal programs would panic for it, but we want to avoid panic at all cost,
    /// so this error exists.
    Unreachable,
    /// A pattern that is not handled by our algorithm yet.
    Unimplemented,
    /// An expected failure, something like "we don't handle this by design",
    /// that worth recording.
    OutOfScope,
}

type NodeId = usize;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Location<'tcx> {
    id: NodeId,
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
