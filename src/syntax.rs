use rustc::hir::{self, intravisit, ImplItemKind, ItemKind, TraitItemKind};
use rustc::ty::TyCtxt;

#[derive(Debug)]
pub enum FunctionType {
    FreeFn(hir::BodyId),
    ProvidedTraitFn(hir::BodyId),
    ImplFn(hir::BodyId),
}

pub struct SyntaxVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    visiting: Option<(FunctionType, bool)>,
    vec: Vec<(FunctionType, bool)>,
}

impl<'tcx> SyntaxVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        SyntaxVisitor {
            tcx,
            visiting: None,
            vec: Vec::new(),
        }
    }

    pub fn collect_functions(&mut self) {
        use intravisit::Visitor;
        self.tcx
            .hir()
            .krate()
            .visit_all_item_likes(&mut self.as_deep_visitor());
    }

    pub fn vec(&self) -> &Vec<(FunctionType, bool)> {
        &self.vec
    }
}

impl<'tcx> intravisit::Visitor<'tcx> for SyntaxVisitor<'tcx> {
    fn nested_visit_map<'this>(&'this mut self) -> intravisit::NestedVisitorMap<'this, 'tcx> {
        intravisit::NestedVisitorMap::OnlyBodies(self.tcx.hir())
    }

    fn visit_item(&mut self, item: &'tcx hir::Item) {
        if let ItemKind::Fn(_decl, _header, _generics, body_id) = &item.node {
            self.visiting = Some((FunctionType::FreeFn(body_id.clone()), false));
        }

        intravisit::walk_item(self, item);

        if let Some(v) = self.visiting.take() {
            self.vec.push(v);
        }
    }

    fn visit_trait_item(&mut self, trait_item: &'tcx hir::TraitItem) {
        if let TraitItemKind::Method(_method_sig, hir::TraitMethod::Provided(body_id)) =
            &trait_item.node
        {
            self.visiting = Some((FunctionType::ProvidedTraitFn(body_id.clone()), false));
        }

        intravisit::walk_trait_item(self, trait_item);

        if let Some(v) = self.visiting.take() {
            self.vec.push(v);
        }
    }

    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem) {
        if let ImplItemKind::Method(_method_sig, body_id) = &impl_item.node {
            self.visiting = Some((FunctionType::ImplFn(body_id.clone()), false));
        }

        intravisit::walk_impl_item(self, impl_item);

        if let Some(v) = self.visiting.take() {
            self.vec.push(v);
        }
    }

    fn visit_block(&mut self, block: &'tcx hir::Block) {
        if let Some(visiting) = self.visiting.as_mut() {
            use hir::BlockCheckMode::*;
            match block.rules {
                DefaultBlock => (),
                UnsafeBlock(_) => visiting.1 = true,
                _ => panic!("push/pop unsafe should not exist"),
            }
        }

        intravisit::walk_block(self, block);
    }
}
