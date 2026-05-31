# Harbor

Harbor is a native Rust app for GitHub.com pull request workflows. It helps you review changes, track checks, inspect workflow logs and manage review threads.

It is designed for fast review workflows: repository switching, pull request
inboxes, changed file navigation, diff review threads, pending review
submission, workflow runs, logs, checks, local PR worktrees, and editor handoff.

## Status

Harbor is early software. The core app builds and has a growing test suite, but
it is not packaged for end-user installation yet.

## Requirements

- Rust `1.90` or newer
- A working system toolchain for GPUI
- GitHub CLI, if you want to authenticate through `gh`

## Run

```bash
cargo run -p harbor-app
```

On first launch, Harbor asks you to connect GitHub.

The easiest path is GitHub CLI auth:

```bash
gh auth login
```

Then choose **Use GitHub CLI** in Harbor.

OAuth device login is also supported. To use it, create a GitHub OAuth app and
start Harbor with a client id:

```bash
HARBOR_GITHUB_OAUTH_CLIENT_ID=your_client_id cargo run -p harbor-app
```

The OAuth flow requests `repo` and `read:org` scopes.

## Development

Run the standard checks before handing off changes:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets
```

There is also a local clippy helper:

```bash
script/clippy
```

## Workspace Layout

- `crates/app`: GPUI application entrypoint and startup wiring
- `crates/ui`: workspace UI, panels, commands, rendering, and interaction state
- `crates/domain`: stable app models independent from GitHub wire formats
- `crates/github`: GitHub REST/GraphQL clients, DTOs, pagination, and transports
- `crates/git`: local Git repository and worktree operations
- `crates/storage`: SQLite cache, recent repositories, and persisted UI state
- `crates/sync`: background refresh policy and inbox change detection
- `crates/logs`: GitHub Actions log parsing

## License

Harbor is licensed under the MIT License. See [LICENSE](LICENSE).

Third-party dependencies are distributed under their own licenses.
