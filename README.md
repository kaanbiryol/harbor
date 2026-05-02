# Harbor

Harbor is a macOS-first native GitHub pull request cockpit built with Rust, GPUI, and `gpui-component`.

The first milestone is a fast native shell with fake data:

- Three-column PR cockpit layout.
- Keyboard-first command registry.
- Fake pull request inbox.
- Fake PR details and changed files.
- Placeholder diff, checks, actions, logs, and command palette panels.
- Optional real open PR loading through GitHub CLI when `HARBOR_REPO=owner/repo` is set.

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
