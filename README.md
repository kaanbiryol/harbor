# Harbor

Harbor is a macOS-first native GitHub pull request cockpit built with Rust, GPUI, and `gpui-component`.

The first milestone is a fast native shell with fake data:

- Three-column PR cockpit layout.
- Keyboard-first command registry.
- Fake pull request inbox.
- Fake PR details and changed files.
- Placeholder diff, checks, actions, logs, and command palette panels.
- Optional real open PR loading through GitHub CLI when `HARBOR_REPO=owner/repo` is set.
- Selected real PR detail and changed-file loading through GitHub CLI.
- Diff preview for the active changed file, with a clear missing-patch fallback for large or binary files.
- Structured unified diff rendering with hunk headers, old/new line numbers, and added/removed/context styling.
- Keyboard commands for changed-file navigation, hunk navigation, copying the active path, and opening the GitHub files view.
- Check runs and workflow runs for the selected PR head SHA.

## Workspace

```text
crates/
  app/       native GPUI application entrypoint
  ui/        GPUI/gpui-component views and command wiring
  domain/    stable app-level domain models
  github/    GitHub transport abstraction
  git/       local git integration boundary
  storage/   persistence boundary
  sync/      background refresh boundary
  logs/      log parsing/rendering model
```

## Development

```bash
cargo fmt --all
cargo test --workspace
cargo run -p harbor-app
```

To load real open pull requests from GitHub through `gh api`:

```bash
HARBOR_REPO=owner/repo cargo run -p harbor-app
```
