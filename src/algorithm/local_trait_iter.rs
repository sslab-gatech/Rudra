use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::HirId;

use crate::prelude::*;

pub struct LocalTraitIter {
    inner: std::vec::IntoIter<HirId>,
}

impl LocalTraitIter {
    pub fn new<'tcx>(ccx: CruxCtxt<'tcx>, trait_def_id: DefId) -> Self {
        let local_trait_impl_map = ccx.tcx().all_local_trait_impls(LOCAL_CRATE);
        let impl_id_vec = local_trait_impl_map
            .get(&trait_def_id)
            .map(Clone::clone)
            .unwrap_or(Vec::new());
        LocalTraitIter {
            inner: impl_id_vec.into_iter(),
        }
    }
}

impl Iterator for LocalTraitIter {
    type Item = HirId;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
