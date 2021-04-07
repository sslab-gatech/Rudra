//! Identify generic parameters that only show up within PhantomType<_>.

use super::*;

/// For a given ADT (struct, enum, union),
/// return the indices of `T`s that are only inside `PhantomData<T>`.
pub fn phantom_indices<'tcx>(tcx: TyCtxt<'tcx>, adt_ty: Ty<'tcx>) -> Vec<u32> {
    // Store indices of gen_params that are in/out of `PhantomData<_>`
    let (mut in_phantom, mut out_phantom) = (FxHashSet::default(), FxHashSet::default());

    if let ty::TyKind::Adt(adt_def, substs) = adt_ty.kind {
        for variant in &adt_def.variants {
            for field in &variant.fields {
                let field_ty = field.ty(tcx, substs);

                let mut walker = field_ty.walk();
                while let Some(node) = walker.next() {
                    if let GenericArgKind::Type(inner_ty) = node.unpack() {
                        if inner_ty.is_phantom_data() {
                            walker.skip_current_subtree();

                            for x in inner_ty.walk() {
                                if let GenericArgKind::Type(ph_ty) = x.unpack() {
                                    if let ty::TyKind::Param(ty) = ph_ty.kind {
                                        in_phantom.insert(ty.index);
                                    }
                                }
                            }
                            continue;
                        }

                        if let ty::TyKind::Param(ty) = inner_ty.kind {
                            out_phantom.insert(ty.index);
                        }
                    }
                }
            }
        }
    }

    // Check for params that are both inside & outside of `PhantomData<_>`
    let in_phantom = in_phantom
        .into_iter()
        .filter(|e| !out_phantom.contains(e))
        .collect();

    return in_phantom;
}
