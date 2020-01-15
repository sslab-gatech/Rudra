use std::fmt;

use rustc::mir;
use rustc::ty::Instance;

pub enum MirBody<'tcx> {
    Static(mir::ReadOnlyBodyAndCache<'tcx, 'tcx>),
    Foreign(Instance<'tcx>),
    Virtual(Instance<'tcx>),
    Unknown(Instance<'tcx>),
    NotAvailable(Instance<'tcx>),
}

impl<'tcx> MirBody<'tcx> {
    pub fn body(&self) -> Option<&'tcx mir::Body<'tcx>> {
        if let MirBody::Static(body) = self {
            Some(&body)
        } else {
            None
        }
    }
}

impl<'tcx> fmt::Debug for MirBody<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MirBody::Static(_) => write!(f, "static body"),
            MirBody::Foreign(instance) => write!(f, "Foreign instance {:?}", instance),
            MirBody::Virtual(instance) => write!(f, "Virtual instance {:?}", instance),
            MirBody::Unknown(instance) => write!(f, "Unknown instance {:?}", instance),
            MirBody::NotAvailable(instance) => {
                write!(f, "MIR not avaiable for instance {:?}", instance)
            }
        }
    }
}
