//! A various utility iterators that iterate over Rustc internal items.
//! Many of these internally use `Vec`. Directly returning that `Vec` might be
//! more performant, but we are intentionally trying to hide the implementation
//! detail here.

use rustc_hir::def_id::{DefId, LocalDefId};

use crate::prelude::*;

/// Given a trait `DefId`, this iterator returns `HirId` of all local impl blocks
/// that implements that trait.
pub struct LocalTraitIter {
    inner: std::vec::IntoIter<LocalDefId>,
}

impl LocalTraitIter {
    pub fn new<'tcx>(rcx: RudraCtxt<'tcx>, trait_def_id: DefId) -> Self {
        let local_trait_impl_map = rcx.tcx().all_local_trait_impls(());
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
    type Item = LocalDefId;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
