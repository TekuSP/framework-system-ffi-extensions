---
name: framework-dotnet-ffi-standalone-repo
description: 'Maintain the standalone future_git repository for the Framework System .NET FFI. Use when updating the framework-system submodule, keeping Cargo and workspace boundaries correct, maintaining CI or release workflows, cleaning parent-repo references after a split, or packaging the FFI as its own repository.'
argument-hint: 'What future_git repo or packaging task do you need?'
---

# Framework .NET FFI Standalone Repo

## When to Use

- Maintain `future_git` as the source-of-truth standalone FFI repo
- Update or inspect the `framework-system` submodule
- Fix path dependencies or Cargo workspace-boundary problems
- Update CI, release workflows, README, or packaging after repo layout changes
- Clean up parent-repo references when the FFI is split out of the main workspace

## Primary Artifacts

- `Cargo.toml`
- `README.md`
- `FFI_NOTES.md`
- `FRAMEWORK_DOTNET_CHANGES_TO_BE_MADE.md`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.gitmodules`
- `framework-system/`

## Procedure

1. Confirm repository ownership and layout.
   - Treat `future_git/` as the active home of the FFI.
   - Confirm the upstream `framework-system` checkout exists as a git submodule.

2. Keep Cargo boundaries correct.
   - The standalone repo must have its own `Cargo.toml` and an empty `[workspace]` table so Cargo does not inherit an outer workspace.
   - Keep the `framework_lib` path dependency pointed at `framework-system/framework_lib`.

3. Keep upstream drift explicit.
   - If clean upstream lacks helper APIs the FFI needs, inline the smallest necessary compatibility logic locally and document why.
   - Avoid silently depending on unmerged parent-repo changes.

4. Maintain packaging surfaces together.
   - Update README, `FFI_NOTES.md`, CI, release workflows, and ignore rules in the same change when layout or ownership changes.
   - Keep release artifacts aligned with the actual native outputs and generated `NativeMethods.g.cs`.

5. Validate downstream managed usage.
   - For ABI-sensitive changes, review `https://github.com/TekuSP/framework-dotnet` as a downstream consumer of this repository.
   - If the FFI change is likely to break managed wrappers, snapshots, exception mapping, or helper extensions there, record the required follow-up in `FRAMEWORK_DOTNET_CHANGES_TO_BE_MADE.md`.

6. Clean parent-repo references when splitting.
   - Remove workspace membership from the parent repo.
   - Remove parent README and CI references to the in-tree FFI crate.
   - Ignore the nested standalone repo from the outer `.gitignore` if the repos remain colocated.

7. Validate the standalone repo.
   - Run `cargo build --manifest-path future_git/Cargo.toml`
   - Run `cargo fmt --manifest-path future_git/Cargo.toml --all -- --check`
   - Run `cargo clippy --manifest-path future_git/Cargo.toml -- -D warnings`
   - Validate editor/YAML errors for CI and release workflow files

## Decision Points

- Upstream merge unlikely:
  - Keep the FFI in `future_git` with upstream as a submodule.

- Clean upstream missing a helper used by the FFI:
  - Prefer a narrow local compatibility layer over broad upstream patch carry.

- Parent repo and standalone repo live side by side:
  - Keep the parent repo clean by removing stale references and ignoring `future_git/`.

- Need distributable artifacts:
  - Use standalone CI for build verification and a separate release workflow for tagged packages.

## Quality Gates

- The standalone repo builds against the upstream submodule, not hidden parent-repo state.
- Cargo workspace boundaries are explicit and stable.
- CI and release workflows match the actual repo layout.
- Managed downstream compatibility has been reviewed for ABI-sensitive changes and required follow-up is tracked if needed.
- Parent-repo references are removed if the FFI is split out.
- Repo docs explain ownership, submodule usage, and release packaging clearly.

## Common Outcomes

- a clean standalone FFI repository with upstream tracked as a submodule
- reproducible packaging and release automation
- fewer hidden dependencies on unmerged upstream changes
- clearer repo ownership and maintenance boundaries
