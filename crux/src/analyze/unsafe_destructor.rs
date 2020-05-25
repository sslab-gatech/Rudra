//! Unsafe destructor detector
use rustc_hir::def_id::LOCAL_CRATE;

use crate::error::{Error, Result};
use crate::prelude::*;

pub struct UnsafeDestructor<'tcx> {
    ccx: CruxCtxt<'tcx>,
}

impl<'tcx> UnsafeDestructor<'tcx> {
    pub fn new(ccx: CruxCtxt<'tcx>) -> Self {
        UnsafeDestructor { ccx }
    }

    pub fn analyze(&mut self) -> Result<'tcx, ()> {
        // `key` is trait
        // `value` is impls
        let local_trait_map = self.ccx.tcx().all_local_trait_impls(LOCAL_CRATE);

        for (key, value) in local_trait_map.iter() {
            dbg!(key);
            dbg!(value);
        }

        Ok(())
    }
}
