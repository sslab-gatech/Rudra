use rustc::hir::{self, intravisit, ImplItemKind, ItemKind, TraitItemKind};
use rustc::ty::TyCtxt;
use syntax::source_map::Span;

#[derive(Debug)]
pub enum FunctionSignature {
    FreeFn,
    ProvidedTraitFn,
    ImplFn,
}

#[derive(Debug)]
pub struct FunctionMemo {
    sig: FunctionSignature,
    header: hir::FnHeader,
    body_id: hir::BodyId,
    contains_unsafe: bool,
}

impl FunctionMemo {
    fn new(sig: FunctionSignature, header: hir::FnHeader, body_id: hir::BodyId) -> Self {
        FunctionMemo {
            sig,
            header,
            body_id,
            contains_unsafe: false,
        }
    }
}

// 'a: analyze function lifetime
// 'tcx: TyCtxt lifetime
pub struct FunctionCollector<'a, 'tcx> {
    tcx: &'a TyCtxt<'tcx>,
    visiting: Option<FunctionMemo>,
    functions: Vec<FunctionMemo>,
}

impl<'a, 'tcx> FunctionCollector<'a, 'tcx> {
    pub fn new(tcx: &'a TyCtxt<'tcx>) -> Self {
        FunctionCollector {
            tcx,
            visiting: None,
            functions: Vec::new(),
        }
    }

    pub fn collect_functions(&mut self) {
        use intravisit::Visitor;
        self.tcx
            .hir()
            .krate()
            .visit_all_item_likes(&mut self.as_deep_visitor());
    }

    pub fn functions(&self) -> &Vec<FunctionMemo> {
        &self.functions
    }
}

impl<'a, 'tcx> intravisit::Visitor<'tcx> for FunctionCollector<'a, 'tcx> {
    fn nested_visit_map<'this>(&'this mut self) -> intravisit::NestedVisitorMap<'this, 'tcx> {
        intravisit::NestedVisitorMap::OnlyBodies(self.tcx.hir())
    }

    fn visit_item(&mut self, item: &'tcx hir::Item) {
        if let ItemKind::Fn(_decl, header, _generics, body_id) = &item.kind {
            self.visiting = Some(FunctionMemo::new(
                FunctionSignature::FreeFn,
                header.clone(),
                body_id.clone(),
            ));
        }

        intravisit::walk_item(self, item);

        if let Some(v) = self.visiting.take() {
            self.functions.push(v);
        }
    }

    fn visit_trait_item(&mut self, trait_item: &'tcx hir::TraitItem) {
        if let TraitItemKind::Method(method_sig, hir::TraitMethod::Provided(body_id)) =
            &trait_item.kind
        {
            self.visiting = Some(FunctionMemo::new(
                FunctionSignature::ProvidedTraitFn,
                method_sig.header.clone(),
                body_id.clone(),
            ));
        }

        intravisit::walk_trait_item(self, trait_item);

        if let Some(v) = self.visiting.take() {
            self.functions.push(v);
        }
    }

    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem) {
        if let ImplItemKind::Method(method_sig, body_id) = &impl_item.kind {
            self.visiting = Some(FunctionMemo::new(
                FunctionSignature::ImplFn,
                method_sig.header.clone(),
                body_id.clone(),
            ));
        }

        intravisit::walk_impl_item(self, impl_item);

        if let Some(v) = self.visiting.take() {
            self.functions.push(v);
        }
    }

    fn visit_block(&mut self, block: &'tcx hir::Block) {
        if let Some(visiting) = self.visiting.as_mut() {
            use hir::BlockCheckMode::*;
            match block.rules {
                DefaultBlock => (),
                UnsafeBlock(_) => visiting.contains_unsafe = true,
                _ => panic!("push/pop unsafe should not exist"),
            }
        }

        intravisit::walk_block(self, block);
    }
}

// 'a: analyze function lifetime
// 'tcx: TyCtxt lifetime
pub struct ModuleCollector<'a, 'tcx> {
    tcx: &'a TyCtxt<'tcx>,
    modules: Vec<Span>,
}

impl<'a, 'tcx> ModuleCollector<'a, 'tcx> {
    pub fn new(tcx: &'a TyCtxt<'tcx>) -> Self {
        ModuleCollector {
            tcx,
            modules: Vec::new(),
        }
    }

    pub fn collect_modules(&mut self) {
        use intravisit::Visitor;
        self.tcx
            .hir()
            .krate()
            .visit_all_item_likes(&mut self.as_deep_visitor());
    }

    pub fn modules(&self) -> &Vec<Span> {
        &self.modules
    }
}

impl<'a, 'tcx> intravisit::Visitor<'tcx> for ModuleCollector<'a, 'tcx> {
    fn nested_visit_map<'this>(&'this mut self) -> intravisit::NestedVisitorMap<'this, 'tcx> {
        intravisit::NestedVisitorMap::OnlyBodies(self.tcx.hir())
    }

    fn visit_mod(&mut self, m: &'tcx hir::Mod, span: Span, n: hir::HirId) {
        self.modules.push(span);
        intravisit::walk_mod(self, m, n);
    }
}
