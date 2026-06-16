This repository is a TypeScript-Go to Rust port.

## Source Of Truth

- Use `vendor/typescript-go` as the primary source of truth for file/code
  structure, function names, port shape, comments, behavior, diagnostics, type
  inference, emit semantics, expected baselines, accepted diffs, and test
  harness behavior.
- Treat TypeScript-Go as the behavioral source of truth, not a requirement to
  copy Go's runtime architecture exactly. Rust ownership, storage, lifetime,
  concurrency, and memory-management constraints may require different internal
  architecture. For example, the Rust AST arena/store model is allowed to
  diverge from TypeScript-Go pointer-style AST internals as long as externally
  observable compiler behavior matches upstream.
- Do not invent Rust-only behavior to make tests pass. Port upstream behavior.
- JSDoc support has been intentionally removed from this port. Do not
  reintroduce JSDoc AST/parser/checker/LS behavior or JSDoc tests unless the
  project direction changes explicitly.

## Refactor Style

- Large breaking refactors are acceptable.
- When the architecture is wrong, prefer deleting legacy code and fixing the
  fallout over keeping compatibility wrappers.
- Do not add fallbacks, cheats, test-only shortcuts, or workaround layers.
- It is acceptable if a clean architectural change temporarily creates many
  compile errors. Keep going and fix them directly.
- Avoid preserving old APIs just to reduce churn. If an API keeps the wrong
  model alive, remove it.

## Preferred Engineering Direction

- Prefer clean long-term architecture over the smallest safe patch.
- Prefer explicit Rust ownership/handles/side tables over pointer-style
  emulation, leaks, or hidden global state.
- TypeScript-Go behavior is the source of truth, but Rust-native architecture
  may diverge from TypeScript-Go internals when that is cleaner or necessary for
  Rust correctness.
- For perf work, profile first, then make structural fixes that match upstream
  behavior. Do not leave speculative "optimization" attempts if they do not
  help.
- If a change is only useful for the test harness but not real compiler/project
  work, call that out honestly before keeping it.
- Baselines are outputs to compare against upstream behavior, not source files
  to hand-edit unless the runner/writer itself is being fixed.

## Workflow

- For generated code, update the generator/source model first. Do not hand-edit
  generated output as the real fix unless the generator itself is being fixed.
