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
use rustc_span::hygiene::{AstPass, ExpnKind};

declare_clippy_lint! {
    /// ### What it does
    /// Checks for modules of the current crate that depend on each other in a cycle.
    ///
    /// A module depends on another module when it names an item defined there, calls a
    /// method or an operator implemented there, or reads a field of a type defined there.
    /// The lint is emitted at every such reference that is part of a cycle, so `#[allow]`
    /// and `#[expect]` apply to one reference at a time.
    ///
    /// A module written by a macro counts as part of the module that contains the macro
    /// call. References from the `main` function that `--test` adds are ignored.
    /// A cycle of only a module and its direct child is not reported. A parent can call
    /// a helper in a child module that uses the parent's types. Longer cycles that pass
    /// through a parent and child are still reported, as are cycles with a grandchild.
    ///
    /// ### Why restrict this?
    /// Modules in a cycle cannot be understood, tested, or moved to a separate crate one at
    /// a time. An acyclic module graph keeps the layering of a crate explicit.
    ///
    /// ### Known problems
    /// The lint does not record a dependency that goes only through a call the compiler
    /// adds, such as:
    /// - the `From` conversion in `?`
    /// - `IntoIterator` in a `for` loop
    /// - an automatic `Deref`
    /// - `Display` through `format!`
    /// - a local `From` implementation reached through `.into()`, which resolves to the
    ///   blanket `Into` implementation of the standard library
    ///
    /// A generic or dynamic trait call does not select a concrete implementation. Only
    /// its dependency on the trait is recorded.
    ///
    /// The lint checks only the active configuration. Code excluded by `#[cfg]` is not
    /// part of the graph. Check each feature configuration that the crate supports.
    ///
    /// To adopt the lint in a crate with existing cycles, use `#[expect]` on individual
    /// references or modules. A module-level expectation also covers new cycles from
    /// that module. An expectation for a feature-specific cycle needs a matching
    /// `#[cfg_attr]`, or it will be unfulfilled when that feature is disabled.
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
    /// Every reference from one module to another, keyed by `(from, to)`.
    edges: FxIndexMap<(LocalDefId, LocalDefId), FxIndexMap<HirId, Span>>,
}

impl ModuleCycles {
    fn record(&mut self, tcx: TyCtxt<'_>, source: HirId, target: DefId, span: Span) {
        let Some(target) = target.as_local() else {
            return;
        };
        if generated_by_test_harness(tcx.def_span(source.owner)) {
            return;
        }
        let from = enclosing_module(tcx, source.owner.def_id);
        let to = enclosing_module(tcx, target);
        if from != to {
            self.edges.entry((from, to)).or_default().entry(source).or_insert(span);
        }
    }

    /// Records the implementation that a call of the trait method `target` selects.
    fn record_implementation(&mut self, cx: &LateContext<'_>, expr: &Expr<'_>, target: DefId) {
        let args = cx.typeck_results().node_args(expr.hir_id);
        // `try_resolve` panics when the arguments do not match the generics of `target`.
        if cx.tcx.def_kind(target) == DefKind::AssocFn
            && args.len() == cx.tcx.generics_of(target).count()
            && let Ok(Some(instance)) = Instance::try_resolve(cx.tcx, cx.typing_env(), target, args)
        {
            self.record(cx.tcx, expr.hir_id, instance.def_id(), expr.span);
        }
    }
}

/// The module that contains `def_id`, or `def_id` itself if it is a module.
///
/// A module written by a macro counts as part of the module that contains it. For example,
/// the `PyO3` crate writes a `mod f` for each `#[pyfunction] fn f`, and that module refers
/// back to its parent, so every binding would otherwise be a cycle.
fn enclosing_module(tcx: TyCtxt<'_>, mut def_id: LocalDefId) -> LocalDefId {
    while tcx.def_kind(def_id) != DefKind::Mod
        || (!def_id.is_top_level_module() && tcx.def_span(def_id).from_expansion())
    {
        def_id = tcx.local_parent(def_id);
    }
    def_id
}

/// Whether `span` comes from the `main` function that `--test` adds to the crate root.
/// That function refers to every test, so it would put each test module in a cycle.
fn generated_by_test_harness(span: Span) -> bool {
    let mut context = span.ctxt();
    while !context.is_root() {
        let expansion = context.outer_expn_data();
        if matches!(expansion.kind, ExpnKind::AstPass(AstPass::TestHarness)) {
            return true;
        }
        context = expansion.call_site.ctxt();
    }
    false
}

/// For every node reachable from `start`, the node before it on a shortest path from `start`.
fn shortest_paths_from(
    graph: &VecGraph<usize>,
    num_nodes: usize,
    start: usize,
    excluded_first_step: Option<usize>,
) -> Vec<Option<usize>> {
    let mut previous = vec![None; num_nodes];
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        for &next in graph.successors(node) {
            if !(node == start && Some(next) == excluded_first_step) && next != start && previous[next].is_none() {
                previous[next] = Some(node);
                queue.push_back(next);
            }
        }
    }
    previous
}

fn is_direct_parent(tcx: TyCtxt<'_>, parent: LocalDefId, child: LocalDefId) -> bool {
    !child.is_top_level_module() && enclosing_module(tcx, tcx.local_parent(child)) == parent
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
        }
        // A call of a trait method also depends on the module of the selected implementation.
        // `qpath_res` also covers a fully qualified call such as `Action::run(value)`.
        let called = match &expr.kind {
            ExprKind::Path(qpath) => cx.qpath_res(qpath, expr.hir_id).opt_def_id(),
            _ => typeck.type_dependent_def_id(expr.hir_id),
        };
        if let Some(target) = called {
            self.record_implementation(cx, expr, target);
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
        // Ordinary edges share a search. Parent-child edges exclude the immediate return
        // edge so the diagnostic names a longer cycle when one exists.
        let mut paths = FxHashMap::default();

        for (&(from, to), references) in &self.edges {
            let parent_child = is_direct_parent(cx.tcx, from, to) || is_direct_parent(cx.tcx, to, from);
            let (from, to) = (index(from), index(to));
            if sccs.scc(from) != sccs.scc(to) {
                continue;
            }
            let excluded_first_step = parent_child.then_some(from);
            let previous = paths
                .entry((to, excluded_first_step))
                .or_insert_with(|| shortest_paths_from(&graph, modules.len(), to, excluded_first_step));
            if previous[from].is_none() {
                continue;
            }
            let mut cycle = vec![from];
            let mut node = from;
            while node != to {
                node = previous[node].unwrap();
                cycle.push(node);
            }
            cycle.push(from);
            cycle.reverse();
            let cycle = cycle
                .into_iter()
                .map(|module| format!("`{}`", name(module)))
                .collect::<Vec<_>>()
                .join(" -> ");
            for (&hir_id, &span) in references {
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
}
