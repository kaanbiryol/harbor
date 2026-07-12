# AGENTS.md

Use `gpui-component` for buttons, tabs, dock/panel layouts, virtualized lists/tables, markdown or simple HTML rendering, and other standard controls. Use raw GPUI directly where lower-level layout, rendering, or performance control is clearer.

## Engineering Priorities

Performance, speed, and smoothness are top-level product requirements. Low-latency input, instant navigation, no visible jank, and no blocking work on the UI path.

Treat responsiveness as an architectural constraint:

- Keep the UI thread free of network, filesystem, Git, SQLite, parsing, and log-processing work.
- Prefer background tasks, incremental loading, cancellation, and progressive rendering.
- Avoid avoidable allocations and cloning in hot paths.
- Use stable IDs and narrow state updates so selection changes, list movement, and tab switches do not trigger broad rerenders.
- Virtualize large views from the start rather than after they become slow.
- Measure performance-sensitive changes with profiling or targeted benchmarks instead of guessing.
- Prefer simple, predictable data structures until profiling proves a more complex one is needed.

## Rust Standards

Write idiomatic, maintainable Rust that can scale with future development.

- Try to achieve your goals with as minimal code as possible.
- Follow standard Rust formatting with `cargo fmt`.
- Keep `cargo clippy --workspace --all-targets` clean unless a lint is intentionally allowed with a narrow explanation.
- Prefer strong domain types over stringly typed state.
- Use `Result` and typed errors for recoverable failures; avoid panics in application code.
- Avoid `unwrap` and `expect` outside tests, examples, and process startup paths where failure is unrecoverable and the message is useful.
- Keep ownership and borrowing straightforward. Do not introduce shared mutability unless it is the clearest design.
- Keep modules small and cohesive. Public APIs should be intentional, documented when non-obvious, and hard to misuse.
- Use traits at architecture boundaries, such as GitHub transport, storage, sync, and local Git integration. Do not add abstractions without a concrete boundary or testability benefit.
- Keep async code explicit about cancellation, ordering, and UI handoff. Do not block async executors with synchronous heavy work.
- Add focused tests around domain mapping, diff parsing, review-thread mapping, storage schema initialization, and command behavior.
- Use `tracing` or an equivalent structured logging approach for diagnostics once runtime code exists.

## Target Architecture

Use a layered Rust workspace. The initial workspace should be shaped like this:

```text
crates/
  app/       GPUI application entrypoint, window setup, command registry, app state wiring
  ui/        reusable GPUI/gpui-component components, layout primitives, lists, tabs, diff/log viewers
  domain/    app-level types independent from GitHub JSON
  github/    GitHub client, DTOs, REST, GraphQL, pagination, auth transport abstractions
  git/       local git operations, checkout, worktrees, local diff, editor/browser opening
  storage/   SQLite cache, settings, recent repos, persisted UI state
  sync/      background refresh workers, cache invalidation, subscriptions
  logs/      GitHub Actions log download, extraction, parsing, searchable log model
```

Keep GitHub DTOs out of `ui`. Map API responses into stable domain models first.

## Crate Boundaries

- `domain` owns stable app models and should not depend on GitHub DTOs, GPUI, SQLite, or local Git process details.
- `github` owns GitHub transport, DTOs, pagination, and API-to-domain mapping.
- `ui` should render domain models and call app/workspace commands rather than reaching into transport or storage details.
- `app` wires crates together and should stay thin.
- Avoid adding cross-crate dependencies that bypass these boundaries unless there is a clear architectural reason.

## Performance Requirements

- Initial window should appear in under one second after binary start.
- Keyboard input and selection movement should feel immediate, including in large PR, file, diff, and log views.
- Keep animations and panel transitions smooth under normal repository sizes.
- Large PR/file/log lists must be virtualized.
- Network refresh must not block UI interaction.
- Diff rendering must not block the main UI.
- Do not render 100k log lines naively.
- Logs should support search and failed-step navigation.

## Validation

Once the Rust workspace exists, use these checks before handing work back:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets
```

If a check is unavailable or too slow during early scaffolding, say exactly what was skipped and why.

## Repository Workflow

- Keep commits in Conventional Commits format.
- Commit messages must be entirely lowercase.
- Use only these commit types: `fix`, `feat`, `chore`, `refactor`, `perf`, `build`, `ci`.
- Use `ios` or `android` scope only when a change affects one platform. Omit scope for cross-platform or repo-wide changes.
- Do not add `Co-Authored-By` trailers.
- Use `gh` CLI for GitHub operations.
- Keep generated or exploratory notes out of source directories unless they are intentional project documentation.

## Rust Coding Guidelines

* Prioritize code correctness and clarity. Treat responsiveness as part of correctness for UI paths and user-facing workflows.
* Do not write organizational or comments that summarize the code. Comments should only be written in order to explain "why" the code is written in some way in the case there is a reason that is tricky / non-obvious.
* Prefer implementing functionality in existing files unless it is a new logical component. Avoid creating many small files.
* Avoid using functions that panic like `unwrap()`, instead use mechanisms like `?` to propagate errors.
* Be careful with operations like indexing which may panic if the indexes are out of bounds.
* Never silently discard errors with `let _ =` on fallible operations. Always handle errors appropriately:
  - Propagate errors with `?` when the calling function should handle them
  - Use `.log_err()` or similar when you need to ignore errors but want visibility
  - Use explicit error handling with `match` or `if let Err(...)` when you need custom logic
  - Example: avoid `let _ = client.request(...).await?;` - use `client.request(...).await?;` instead
* When implementing async operations that may fail, ensure errors propagate to the UI layer so users get meaningful feedback.
* Never create files with `mod.rs` paths - prefer `src/some_module.rs` instead of `src/some_module/mod.rs`.
* Avoid creative additions unless explicitly requested
* Use full words for variable names (no abbreviations like "q" for "queue")
* Use variable shadowing to scope clones in async contexts for clarity, minimizing the lifetime of borrowed references.
  Example:
  ```rust
  executor.spawn({
      let task_ran = task_ran.clone();
      async move {
          *task_ran.borrow_mut() = true;
      }
  });
  ```

## Timers In Tests

* In GPUI tests, prefer GPUI executor timers over `smol::Timer::after(...)` when you need timeouts, delays, or to drive `run_until_parked()`:
  - Use `cx.background_executor().timer(duration).await` (or `cx.background_executor.timer(duration).await` in `TestAppContext`) so the work is scheduled on GPUI's dispatcher.
  - Avoid `smol::Timer::after(...)` for test timeouts when you rely on `run_until_parked()`, because it may not be tracked by GPUI's scheduler and can lead to "nothing left to run" when pumping.

## Testing And External Services

* Do not add tests that require live GitHub, network access, or user credentials by default.
* Prefer fake transports, local fixtures, and deterministic DTO samples for GitHub behavior.
* Gate any live integration test behind an explicit environment variable and document what service and permissions it needs.
* Do not log GitHub tokens, auth headers, credential-bearing URLs, or raw secrets. Redact sensitive values before passing data to `tracing` or error messages.

## GPUI

GPUI is a UI framework which also provides primitives for state and concurrency management. Prefer documenting project-specific GPUI patterns in this file instead of copying general framework reference material.

### Context And Entity Usage

- Name GPUI app contexts `cx`. When a function also takes a `Window`, pass `window` before `cx`.
- When an `Entity::update`, `Entity::update_in`, `WeakEntity::update`, or `WeakEntity::update_in` closure provides an inner `cx`, use that inner `cx` inside the closure instead of the outer one.
- Avoid updating an entity while it is already being updated; reentrant entity updates can panic.
- Use `WeakEntity` for async callbacks or mutually-referential entities that should not keep each other alive. Handle the `anyhow::Result` returned by weak entity reads and updates.
- Call `cx.notify()` after state changes that affect rendering.
- Store `cx.subscribe` results in a `_subscriptions: Vec<Subscription>` field so subscriptions stay alive for the entity lifetime.

### Concurrency

All use of entities and UI rendering occurs on a single foreground thread.

`cx.spawn(async move |cx| ...)` runs an async closure on the foreground thread. Within the closure, `cx` is `&mut AsyncApp`.

When the outer cx is a `Context<T>`, the use of `spawn` instead looks like `cx.spawn(async move |this, cx| ...)`, where `this: WeakEntity<T>` and `cx: &mut AsyncApp`.

To do work on other threads, `cx.background_spawn(async move { ... })` is used. Often this background task is awaited on by a foreground task which uses the results to update state.

Both `cx.spawn` and `cx.background_spawn` return a `Task<R>`, which is a future that can be awaited upon. If this task is dropped, then its work is cancelled. To prevent this one of the following must be done:

* Awaiting the task in some other async context.
* Detaching the task via `task.detach()` or `task.detach_and_log_err(cx)`, allowing it to run indefinitely.
* Storing the task in a field, if the work should be halted when the struct is dropped.

A task which doesn't do anything but provide a value can be created with `Task::ready(value)`.

### Rendering And Events

- Use `Render` for persistent views and `RenderOnce` with `#[derive(IntoElement)]` for one-shot component values that are immediately turned into elements.
- Prefer `SharedString` for UI text that is cloned into element trees.
- Use `.when(condition, |this| ...)` and `.when_some(option, |this, value| ...)` for conditional element attributes or children when that keeps the tree local and readable.
- Use `cx.listener` for input or action handlers that need to update the entity from the current `Context<T>`.
- Actions are user-facing; keep action doc comments clear because they can be displayed in command UI.
