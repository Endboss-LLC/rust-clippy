# Resume — module cycle lint prototype

Created: 2026-11-06T1814Z
Updated: 2026-11-06T1822Z
Branch: module-cycles-lint

## Goal

Prepare the fork lint for an upstream design discussion without posting upstream.

## Accepted decisions

- Keep the lint off by default in the restriction group.
- Use source `allow` and `expect` attributes, with no exception list or external checker in this fork.
- Exempt cycles of only a module and its direct child. Keep longer cycles through that pair and cycles with a grandchild.
- Keep the existing macro-module normalization and test-harness exclusion.
- Leave Exchanger PR #3414 unchanged.

## Completed

- The follow-up to `76d1b8e72` adds the parent-child rule inside the existing path search.
- Parent-child references remain in the graph. Only the direct return edge is excluded when testing that pair.
- UI fixtures cover the crate root, nested modules, longer cycles, grandchild cycles, a parent outside a sibling cycle, and feature-specific module expectations.
- Macro fixtures now use sibling modules so the parent-child exemption cannot hide their coverage.
- Prototype lint docs explain the exemption, unresolved trait calls, active configurations, and module-wide expectations.
- The author cut pass retained the existing SCC and BFS implementation. No configuration or graph-file subsystem was added.
- The lint patch is 930 added lines from `5ac3102bb`, including prose, snapshots and registration, excluding this internal handoff. The Rust fixtures and lint source account for 667 lines, including lint docs.

## Next action

Run the required fresh Claude simplification review of the final checkpoint, address findings, then remove this internal handoff before any upstream patch.

## Open decisions or blockers

- The required opposite-family review failed immediately with `Claude API rate limit reached`. Do not treat the author review as an independent review.
- Upstream must agree to review AI-created code before a PR. Its current policy also requires human authorship of documentation and diagnostic messages.
- The parent-child exemption is a proposed design choice for upstream discussion, not an upstream-approved rule.
- Existing performance measurements describe the earlier prototype, not this follow-up.

## Last focused verification

- Docker image: `clippy-bench:nightly-2026-10-30`, with `rustc-dev`; repository mounted at `/clippy`, build output at `/target`.
- `TESTNAME=module_cycles_parent_child cargo test --test compile-test` failed before the change with six unwanted cycle diagnostics and passed after it.
- `TESTNAME=module_cycles cargo test` exercised all cycle fixtures and the on/off feature revisions. Formatting, config checks and dogfood completed. The lint-attribute ordering check then found the new fixture's attribute order.
- The fixture's attribute order was corrected. Its two feature revisions, formatting, lint attributes, lint-page, message convention, missing-test-files, profile policy, symbols, versions and workspace tests passed; see `/tmp/clippy-module-cycles-remaining.log`.
- `cargo dev lint clippy_utils -- -- -W clippy::module_cycles --cap-lints=warn` completed on the final source with 232 cycle warnings and no compiler crash; see `/tmp/clippy-module-cycles-smoke.log`.
- Earlier evidence: `/tmp/clippy-parent-child-red.log`, `/tmp/clippy-parent-child-green.log`, `/tmp/clippy-module-cycles-final.log`.

## Private notes for the upstream discussion

These are AI-generated drafting notes for Kevin, not a comment to paste upstream. Write an independent message in your own words. The policy does allow clearly quoted AI material when the human message also stands on its own, but these notes are intended as private reference.

- Link the existing idea, rust-clippy#5782, and the internal fork PR. Disclose that the prototype contains AI-created code, docs and diagnostic text.
- Ask whether a restriction lint is welcome and whether a maintainer would be willing to review an AI-created code contribution under the experiment rules.
- Explain the proposed rule: detect cyclic references within one crate, report each reference at its HIR node, and use ordinary source attributes for existing cycles.
- Explain the parent-child choice with a concrete example: a parent calls a helper in its child, while the helper uses a type from the parent. Exempt that two-module cycle; retain longer cycles.
- Mention that macro-written modules count as their containing module and that compiler-generated test-harness references are ignored.
- State the limits: active configurations only, no inferred dynamic implementations, and missed dependencies through implicit conversions and other compiler-added calls.
- Explain why the existing tools did not meet this project's requirements. Keep the public draft to one paragraph; use the version-specific evidence below for follow-up questions.
- Historical measurements on the earlier prototype used 52 crates and about 297k Rust lines. Three alternating enabled/disabled runs on a shared host had overlapping CPU ranges. This is evidence about that workload, not a zero-cost guarantee or a new measurement of this revision.
- Human authorship is still needed for the lint documentation, comments and diagnostic messages intended for upstream. Disclosure and a reviewer agreement do not waive that requirement.

### Other approaches investigated

- `cargo metadata` describes packages and targets; Graphviz renders a supplied graph. Neither supplies the resolved references within a crate that this check needs.
- The `cargo-modules` 0.28.0 approach inspected imports and types but omitted function bodies, missing the body-only module cycles in our fixtures. Its graph output remained useful for architectural inspection.
- Cargo Bylaw at `7d3cf0fe28d133730193084224cf500ff87b9b7a` used rust-analyzer and supported body analysis, but our fixtures exposed gaps in nested generic types, method calls and macro output. We patched these and adapted package/feature isolation. A full isolated scan then remained too expensive for our check-time budget.
- A standalone rustc-driver prototype established that compiler information could cover the required references. Running that as another mandatory compiler pass would still repeat analysis, so the next prototype put collection inside the Clippy invocations the gate already runs.
- Dylint was discussed as a way to package custom lints, not benchmarked as an implemented alternative. Do not imply that it was ruled out experimentally.
- These are findings about the revisions and integration tested here, not a survey of current tool releases. The earlier analyzer and compiler timings were not an equal-cache, controlled comparison; omit speedup ratios.

### AI-generated private draft

This is a drafting aid, not an upstream-ready comment. Minor editing does not change its AI origin. Use the facts above to write your own message independently.

> Is there still interest in an opt-in module-cycle lint for this issue? We have an internal prototype in https://github.com/Endboss-LLC/rust-clippy/pull/1 and would like feedback on the design before proposing a PR.
>
> We first investigated standalone tools. The `cargo-modules` version we inspected omitted function-body references. Cargo Bylaw needed fixes for nested types, method calls and macro output; even after those fixes, separate analysis across our isolated package and feature configurations exceeded our check-time budget. That led us to reuse rustc's resolved information during the Clippy checks we already run. These findings concern the versions and integration we tested, rather than every standalone approach.
>
> It is a restriction lint that reports individual cyclic references, so normal `allow` and `expect` attributes work. It treats macro-written modules as part of their containing module and ignores test-harness references. One proposed exemption permits a cycle of only a parent and its direct child, while retaining longer cycles through that pair.
>
> The prototype has compiler UI coverage and has been exercised on a larger workspace. It still misses some implicit compiler-added dependencies; the fork documents those limits. Earlier performance measurements are exploratory, not a zero-overhead claim.
>
> The prototype contains AI-created code, documentation and diagnostics. Before any upstream PR, the prose and diagnostics would need human authorship. Would a maintainer be willing to discuss the design and review an AI-created code contribution under the project's experiment rules?

## Pointers

- Fork PR: https://github.com/Endboss-LLC/rust-clippy/pull/1
- Upstream idea: https://github.com/rust-lang/rust-clippy/issues/5782
- Policy: https://forge.rust-lang.org/policies/llm-usage
- Clippy interpretation: https://doc.rust-lang.org/nightly/clippy/development/llm_usage.html
- Feature workflow: `/home/kevin/.codex/skills/claude/feature/SKILL.md`
- Review rubric: `/home/kevin/.codex/skills/claude/review-rubric/SKILL.md`
