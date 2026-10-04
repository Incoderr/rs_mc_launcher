## Project Overview

This project is a native Minecraft launcher written primarily in Rust.

The application uses:

- Rust for application and launcher logic.
- GPUI for the native desktop UI.
- `gpui-component` / GPUI Kit for reusable UI primitives where appropriate.
- Java only as a managed runtime for launching Minecraft. The launcher itself must not depend on Java for its implementation.

The project should remain native-first. Do not introduce Electron, Tauri, WebView, React, Node.js, or another web runtime unless explicitly requested.

## Architecture

Use a feature-first architecture with a UI-independent launcher core.

The main dependency direction is:

```text
app / pages
    ↓
features
    ↓
core
    ↓
platform
```

Dependencies must not point upward.

In particular:

- `core` must not depend on GPUI.
- `platform` must not depend on GPUI.
- Business logic must not live inside GPUI components.
- Pages should compose features rather than own feature logic.
- Features should own their state, actions, services, and feature-specific UI.

## Suggested Project Structure

```text
crates/
├── app/
│   └── src/
│       ├── main.rs
│       ├── app.rs
│       ├── router.rs
│       │
│       ├── pages/
│       │   ├── home.rs
│       │   ├── instances.rs
│       │   └── settings.rs
│       │
│       ├── features/
│       │   ├── auth/
│       │   ├── instances/
│       │   ├── downloads/
│       │   ├── mods/
│       │   ├── modpacks/
│       │   ├── java/
│       │   ├── launch/
│       │   ├── settings/
│       │   └── updater/
│       │
│       ├── shared/
│       │   ├── ui/
│       │   ├── theme/
│       │   └── icons/
│       │
│       └── shell/
│           ├── window.rs
│           └── layout.rs
│
├── minecraft-core/
│   └── src/
│       ├── auth/
│       ├── metadata/
│       ├── version/
│       ├── assets/
│       ├── libraries/
│       ├── natives/
│       ├── launch/
│       └── loader/
│           ├── fabric/
│           ├── forge/
│           └── neoforge/
│
└── platform/
    └── src/
        ├── fs/
        ├── process/
        ├── network/
        ├── paths/
        └── keyring/
```

Do not create crates purely for architectural symmetry.

Start with modules inside the existing crates. Extract a module into a separate crate only when there is a concrete reason, such as independent reuse, dependency isolation, compile-time boundaries, or platform separation.

## Feature Structure

A feature owns functionality rather than merely representing a screen.

A typical feature can contain:

```text
features/instances/
├── mod.rs
├── state.rs
├── actions.rs
├── service.rs
├── model.rs
└── ui/
    ├── instance_card.rs
    ├── instance_list.rs
    └── create_dialog.rs
```

Not every feature needs every file.

Do not create empty architectural layers just to follow this structure.

Prefer the smallest structure that keeps responsibilities clear.

For example, a small feature may simply be:

```text
features/settings/
├── mod.rs
├── state.rs
└── view.rs
```

Split files when they become meaningfully independent.

## Features Are Not Pages

Do not organize business logic around screens.

For example, the Home page may compose:

```text
HomePage
├── auth::AccountSwitcher
├── instances::SelectedInstance
├── launch::PlayButton
└── downloads::DownloadProgress
```

The page coordinates layout and composition.

The individual features own their behavior.

Avoid:

```text
pages/home/
├── account_logic.rs
├── download_logic.rs
├── instance_logic.rs
└── minecraft_launch.rs
```

## Core Rules

`minecraft-core` contains Minecraft-specific logic that can operate without the desktop UI.

Examples:

- Minecraft version metadata.
- Version inheritance.
- Asset indexes.
- Library resolution.
- Native libraries.
- Classpath construction.
- JVM arguments.
- Game arguments.
- Java compatibility requirements.
- Minecraft installation.
- Minecraft process configuration.
- Fabric installation/resolution.
- Forge installation/resolution.
- NeoForge installation/resolution.

Core APIs should use normal Rust types.

Good:

```rust
pub async fn prepare_instance(
    instance: &Instance,
) -> Result<PreparedInstance, PrepareError>
```

Good:

```rust
pub async fn launch(
    instance: &Instance,
    options: LaunchOptions,
) -> Result<GameProcess, LaunchError>
```

Bad:

```rust
pub fn launch(
    cx: &mut gpui::App,
    instance: &Instance,
)
```

GPUI types must never leak into `minecraft-core`.

## Platform Layer

OS-specific operations belong in the platform layer.

Examples:

- Filesystem operations.
- Launcher directories.
- Process spawning.
- Process inspection.
- Opening directories or URLs.
- Credential/keyring access.
- OS detection.
- Architecture detection.
- Platform-specific Java discovery.

Expose typed APIs rather than spreading conditional compilation throughout the application.

Prefer:

```rust
platform::java::discover()
platform::process::spawn(...)
platform::paths::launcher_data_dir()
```

over repeated:

```rust
#[cfg(target_os = "...")]
```

inside feature/UI code.

## State Management

Prefer feature-local state.

Examples:

```rust
pub struct AuthState {
    pub accounts: Vec<Account>,
    pub active_account: Option<AccountId>,
}
```

```rust
pub struct InstancesState {
    pub instances: Vec<Instance>,
    pub selected: Option<InstanceId>,
}
```

```rust
pub struct DownloadState {
    pub active: Vec<Download>,
    pub queued: Vec<Download>,
}
```

The application root may hold handles to feature state:

```rust
pub struct LauncherApp {
    pub auth: Entity<AuthState>,
    pub instances: Entity<InstancesState>,
    pub downloads: Entity<DownloadState>,
    pub settings: Entity<SettingsState>,
}
```

Avoid creating one giant mutable `AppState` containing every field in the application.

State should have a clear owner.

## Data Flow

Prefer this general direction:

```text
User interaction
      ↓
GPUI component
      ↓
feature action/method
      ↓
service/use case
      ↓
core/platform
      ↓
state update
      ↓
GPUI render
```

Do not introduce Redux-style actions/reducers for trivial state changes.

Simple state changes can remain methods:

```rust
impl InstancesState {
    pub fn select(&mut self, id: InstanceId) {
        self.selected = Some(id);
    }
}
```

Use dedicated services/use cases when an operation involves asynchronous work, multiple dependencies, IO, or meaningful business logic.

## Async Work

Never block the GPUI/UI thread with:

- Network requests.
- File downloads.
- Hash verification.
- Large filesystem scans.
- Archive extraction.
- Minecraft installation.
- Modpack installation.
- Java downloads.
- Process waiting.

Long-running work should execute asynchronously/background where appropriate and report progress through feature state.

Cancellation should be supported for operations where cancellation is useful, especially downloads and installations.

## Downloads

Treat downloads as a reusable subsystem rather than implementing downloading independently inside every feature.

The download system should eventually support:

- Queueing.
- Concurrent downloads.
- Progress.
- Total progress.
- Cancellation.
- Retries.
- Checksums.
- Temporary files.
- Atomic completion.
- Resume support where practical.

Other features should request downloads through this subsystem.

For example:

```text
minecraft install ─┐
java install ──────┤
mod download ──────┼──> DownloadManager
modpack install ───┤
launcher update ───┘
```

## Minecraft Instances

An instance represents an independently configurable Minecraft installation/profile.

Avoid coupling instance identity directly to a filesystem path.

Prefer stable IDs:

```rust
pub struct InstanceId(Uuid);
```

An instance may contain configuration such as:

```rust
pub struct Instance {
    pub id: InstanceId,
    pub name: String,
    pub game_version: String,
    pub loader: Option<Loader>,
    pub java: JavaConfig,
    pub memory: MemoryConfig,
}
```

Exact persistence structures may differ from runtime/domain structures.

Do not expose raw persistence representation throughout the application.

## IDs

Prefer typed IDs instead of raw strings.

Good:

```rust
struct InstanceId(Uuid);
struct AccountId(Uuid);
struct DownloadId(Uuid);
```

Avoid APIs where unrelated IDs are interchangeable:

```rust
fn remove(id: String)
```

## Authentication

Authentication should be isolated behind the auth feature/core API.

Do not expose OAuth implementation details throughout UI code.

Never log:

- Access tokens.
- Refresh tokens.
- Microsoft credentials.
- Minecraft session credentials.
- Secrets.

Sensitive credentials should use the platform credential/keyring mechanism where available rather than plain-text configuration files.

## Java

Treat Java as a managed runtime, not as part of the launcher architecture.

The Java subsystem should be capable of:

- Detecting installed Java runtimes.
- Determining Java version.
- Determining architecture.
- Selecting a compatible runtime for a Minecraft version.
- Downloading a runtime when required.
- Allowing a custom Java path.
- Constructing JVM launch configuration.

Do not assume one Java version works for every Minecraft version.

## Error Handling

Do not use `unwrap()` or `expect()` for recoverable runtime failures.

Use typed errors where the caller needs to distinguish failure categories.

Prefer:

```rust
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("Java runtime was not found")]
    JavaNotFound,

    #[error("Minecraft installation is incomplete")]
    IncompleteInstallation,

    #[error("failed to start process")]
    Process(#[source] std::io::Error),
}
```

Add context at boundaries.

Do not silently ignore errors.

User-facing errors should be understandable and should not expose irrelevant implementation details.

Detailed diagnostic information belongs in logs.

## Logging

Use structured logging, preferably `tracing`.

Prefer:

```rust
tracing::info!(
    instance_id = %instance.id,
    version = %instance.game_version,
    "launching minecraft"
);
```

over:

```rust
println!("Launching!");
```

Never log secrets or authentication tokens.

## UI

Use GPUI as the primary UI framework.

Prefer existing `gpui-component` primitives when they satisfy the requirement.

Create application-specific components on top of those primitives.

For example:

```text
gpui
   ↓
gpui-component
   ↓
shared/ui
   ↓
feature UI
   ↓
pages
```

`shared/ui` should contain genuinely reusable visual primitives.

Examples:

```text
shared/ui/
├── sidebar.rs
├── page_header.rs
├── empty_state.rs
├── loading.rs
└── error_view.rs
```

Feature-specific components remain inside their feature.

For example:

```text
features/instances/ui/instance_card.rs
```

must not be moved into `shared/ui` merely because it is a component.

## Shared Code

Be conservative when moving code into `shared`.

Good candidates:

```text
shared/ui/
shared/theme/
shared/icons/
```

Avoid generic dumping grounds such as:

```text
shared/utils.rs
shared/helpers.rs
shared/common.rs
shared/misc.rs
```

Code should remain in the feature that owns it until there is a real shared abstraction.

Prefer duplication of a few trivial lines over a premature abstraction with unclear ownership.

## Models

Do not create one global `models/` directory containing every application type.

Keep domain types close to the domain that owns them.

Prefer:

```text
auth/model.rs
instances/model.rs
downloads/model.rs
```

Shared core domain types may live in the appropriate core module.

## Services

Do not create a global `services/` directory.

Services should belong to their feature/domain:

```text
features/auth/service.rs
features/instances/service.rs
```

or:

```text
minecraft-core/src/metadata/
minecraft-core/src/launch/
```

depending on responsibility.

## Networking

Keep HTTP implementation details away from UI components.

Bad:

```rust
impl Render for InstancePage {
    // construct HTTP request
    // parse Minecraft metadata
    // write files
}
```

Prefer:

```text
InstancePage
    ↓
InstanceService
    ↓
MinecraftInstaller
    ↓
MetadataClient / DownloadManager
```

Network responses should be converted into domain types where useful.

## Persistence

Persistence must be separated from UI state.

Possible persisted data includes:

- Launcher settings.
- Instances.
- Account metadata.
- Java configuration.
- UI preferences.

Do not serialize GPUI state directly.

Use explicit persistence models or repositories when the distinction becomes useful.

Writes that could corrupt important state should be atomic where practical:

```text
write temporary file
        ↓
flush/validate
        ↓
rename
```

## Dependencies

Before adding a dependency:

1. Check whether the standard library or an existing dependency already solves the problem.
2. Check maintenance/activity of the crate.
3. Check whether it introduces a large dependency tree.
4. Avoid adding overlapping libraries for the same responsibility.
5. Prefer well-established Rust crates for security-sensitive functionality.

Do not introduce architectural frameworks simply to imitate patterns from frontend/web ecosystems.

## Code Style

Prefer explicit, readable Rust over clever abstractions.

Use:

- Strong types.
- Enums for finite states.
- Small focused modules.
- Ownership-based APIs.
- `Result` for fallible operations.
- `Option` for genuinely optional values.
- Exhaustive matching where useful.

Avoid:

- Excessive trait abstraction.
- Deep generic hierarchies.
- Unnecessary macros.
- Stringly typed APIs.
- Global mutable state.
- Hidden side effects.
- Premature abstraction.

## State Machines

For meaningful asynchronous states, prefer enums over collections of booleans.

Bad:

```rust
struct LaunchState {
    loading: bool,
    launching: bool,
    running: bool,
    failed: bool,
}
```

Prefer:

```rust
enum LaunchState {
    Idle,
    Preparing,
    Launching,
    Running {
        pid: u32,
    },
    Failed {
        error: String,
    },
}
```

Impossible states should be difficult to represent.

## Testing

Core logic should be testable without starting GPUI.

Prioritize tests for:

- Version metadata parsing.
- Version inheritance.
- Library rules.
- OS/architecture rules.
- Argument generation.
- Classpath generation.
- Loader metadata.
- Checksum validation.
- Instance serialization.
- Java selection.
- Launch command construction.

Do not require an actual Minecraft launch for tests that can validate command construction independently.

## Agent Workflow

Before changing code:

1. Inspect the existing structure.
2. Identify which feature/domain owns the change.
3. Reuse existing abstractions where appropriate.
4. Check dependency direction.
5. Avoid unrelated refactors.

When implementing a feature:

1. Define the domain/state requirements.
2. Implement or extend core/platform functionality if needed.
3. Implement the feature service/use case.
4. Connect feature state.
5. Build the GPUI presentation.
6. Handle loading, empty, success, failure, and cancellation states where applicable.
7. Add tests for non-trivial core logic.
8. Run formatting, checks, and tests.

Prefer:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

Fix warnings introduced by the change.

Do not modify unrelated code merely to make a patch appear cleaner.

## When Adding New Functionality

Ask these questions in order:

```text
Is this Minecraft domain logic?
        │
        └─ yes → minecraft-core

Is this OS/system integration?
        │
        └─ yes → platform

Does a feature clearly own it?
        │
        └─ yes → features/<feature>

Is it page composition/navigation?
        │
        └─ yes → pages/app/router

Is it genuinely reusable UI?
        │
        └─ yes → shared/ui
```

If ownership is unclear, keep the implementation local until the correct abstraction becomes evident.

## Architectural Priorities

When trade-offs are necessary, prioritize:

1. Correctness.
2. Clear ownership.
3. Separation of UI and launcher logic.
4. Reliability and recoverability.
5. Maintainability.
6. Testability.
7. Performance.
8. Abstraction/reuse.

Performance-sensitive paths should be measured before introducing significant complexity.

## Avoid Overengineering

This project should have strong boundaries without becoming ceremonial Clean Architecture.

Do not automatically introduce:

- Repository traits for every data source.
- Interfaces/traits with only one implementation.
- DTO/domain/view-model copies when the types are identical in purpose.
- Command buses.
- Event buses.
- Dependency injection frameworks.
- Factories for trivial construction.
- Separate crates for tiny modules.

Introduce an abstraction when it solves an actual boundary, testing, ownership, or substitution problem.

The target architecture is:

```text
simple locally
+
strict boundaries globally
```

## Final Rule

When unsure where code belongs, determine who owns the behavior rather than where the code is currently used.

A page using functionality does not mean the page owns it.

A GPUI component displaying data does not mean the component owns the data.

A feature requiring Minecraft functionality does not mean Minecraft logic belongs in the feature.

Keep the dependency direction:

```text
UI → features → core/platform
```

and never make core functionality depend on the UI.