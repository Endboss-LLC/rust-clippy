use std::collections::VecDeque;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_data_structures::fx::{FxHashMap, FxIndexMap, FxIndexSet};
use rustc_data_structures::graph::scc::Sccs;
use rustc_data_structures::graph::vec_graph::VecGraph;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{Expr, ExprKind, HirId, Path};
use rustc_lint::{LateContext, LateLintPass, impl_lint_pass};
use rustc_middle::ty::print::with_crate_prefix;
use rustc_middle::ty::{Instance, TyCtxt};
use rustc_span::Span;

declare_clippy_lint! {
    /// ### What it does
    /// Checks for modules of the current crate that depend on each other in a cycle.
    ///
    /// A module depends on another module when it names an item defined there, calls a
    /// method or an operator implemented there, or reads a field of a type defined there.
    /// The lint is emitted once for every dependency that is part of a cycle, at the
    /// first place where that dependency occurs.
    ///
    /// ### Why restrict this?
    /// Modules in a cycle cannot be understood, tested, or moved to a separate crate one at
    /// a time. An acyclic module graph keeps the layering of a crate explicit.
    ///
    /// ### Example
    /// ```no_run
    /// mod orders {
    ///     pub struct Order;
    ///     pub fn total(order: &Order) -> u32 {
    ///         crate::pricing::price(order)
    ///     }
    /// }
    /// mod pricing {
    ///     pub fn price(_: &crate::orders::Order) -> u32 {
    ///         0
    ///     }
    /// }
    /// ```
    /// Use instead:
    /// ```no_run
    /// mod model {
    ///     pub struct Order;
    /// }
    /// mod orders {
    ///     pub fn total(order: &crate::model::Order) -> u32 {
    ///         crate::pricing::price(order)
    ///     }
    /// }
    /// mod pricing {
    ///     pub fn price(_: &crate::model::Order) -> u32 {
    ///         0
    ///     }
    /// }
    /// ```
    #[clippy::version = "1.101.0"]
    pub MODULE_CYCLES,
    restriction,
    "modules of the current crate that depend on each other in a cycle"
}

impl_lint_pass!(ModuleCycles => [MODULE_CYCLES]);

#[derive(Default)]
pub struct ModuleCycles {
    /// The first reference from one module to another, keyed by `(from, to)`.
    edges: FxIndexMap<(LocalDefId, LocalDefId), (HirId, Span)>,
}

impl ModuleCycles {
    fn record(&mut self, tcx: TyCtxt<'_>, source: HirId, target: DefId, span: Span) {
        let Some(target) = target.as_local() else {
            return;
        };
        let from = enclosing_module(tcx, source.owner.def_id);
        let to = enclosing_module(tcx, target);
        if from != to {
            self.edges.entry((from, to)).or_insert((source, span));
        }
    }
}

/// The module that contains `def_id`, or `def_id` itself if it is a module.
fn enclosing_module(tcx: TyCtxt<'_>, mut def_id: LocalDefId) -> LocalDefId {
    while tcx.def_kind(def_id) != DefKind::Mod {
        def_id = tcx.local_parent(def_id);
    }
    def_id
}

/// The nodes after `from` on a shortest path from `from` to `to`, found by a breadth-first search.
fn shortest_path(graph: &VecGraph<usize>, from: usize, to: usize) -> Vec<usize> {
    let mut previous = FxHashMap::default();
    let mut queue = VecDeque::from([from]);
    while let Some(node) = queue.pop_front()
        && node != to
    {
        for &next in graph.successors(node) {
            if next != from && !previous.contains_key(&next) {
                previous.insert(next, node);
                queue.push_back(next);
            }
        }
    }
    let mut path = vec![to];
    while let Some(&node) = previous.get(path.last().unwrap())
        && node != from
    {
        path.push(node);
    }
    path.reverse();
    path
}

impl<'tcx> LateLintPass<'tcx> for ModuleCycles {
    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &Path<'tcx>, source: HirId) {
        if let Some(target) = path.res.opt_def_id() {
            self.record(cx.tcx, source, target, path.span);
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let typeck = cx.typeck_results();
        if let Some(target) = typeck.type_dependent_def_id(expr.hir_id) {
            self.record(cx.tcx, expr.hir_id, target, expr.span);
            // A call of a trait method also depends on the module of the selected implementation.
            if cx.tcx.def_kind(target) == DefKind::AssocFn
                && let Ok(Some(instance)) =
                    Instance::try_resolve(cx.tcx, cx.typing_env(), target, typeck.node_args(expr.hir_id))
            {
                self.record(cx.tcx, expr.hir_id, instance.def_id(), expr.span);
            }
        }
        if let ExprKind::Field(receiver, _) = expr.kind
            && let Some(adt) = typeck.expr_ty_adjusted(receiver).peel_refs().ty_adt_def()
        {
            self.record(cx.tcx, expr.hir_id, adt.did(), expr.span);
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let modules: FxIndexSet<LocalDefId> = self.edges.keys().flat_map(|&(from, to)| [from, to]).collect();
        let index = |module| modules.get_index_of(&module).unwrap();
        let graph = VecGraph::new(
            modules.len(),
            self.edges.keys().map(|&(from, to)| (index(from), index(to))).collect(),
        );
        let sccs: Sccs<usize, usize> = Sccs::new(&graph);
        let name = |module: usize| with_crate_prefix!(cx.tcx.def_path_str(modules[module]));

        for (&(from, to), &(hir_id, span)) in &self.edges {
            let (from, to) = (index(from), index(to));
            if sccs.scc(from) != sccs.scc(to) {
                continue;
            }
            let cycle = [from, to]
                .into_iter()
                .chain(shortest_path(&graph, to, from))
                .map(|module| format!("`{}`", name(module)))
                .collect::<Vec<_>>()
                .join(" -> ");
            span_lint_hir_and_then(
                cx,
                MODULE_CYCLES,
                hir_id,
                span,
                format!(
                    "module `{}` depends on `{}`, which depends back on it",
                    name(from),
                    name(to)
                ),
                |diag| {
                    diag.note(format!("module cycle: {cycle}"));
                },
            );
        }
    }
}
