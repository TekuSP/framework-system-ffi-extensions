---
name: framework-dotnet-ffi-abi
description: 'Design, review, or extend the Framework System Rust FFI ABI for .NET via csbindgen. Use when changing NativeMethods.g.cs shape, refining Framework...Result records, improving enums or nested structs, reviewing FrameworkByteBuffer ownership, or comparing FFI coverage against framework-system features.'
argument-hint: 'What ABI or coverage change do you need?'
---

# Framework .NET FFI ABI

## When to Use

- Add or change Rust exports intended for `csbindgen`
- Review `NativeMethods.g.cs` and improve the generated C# shape at the ABI layer
- Replace awkward flat bytes or flags with enums, nested records, or clearer result structs
- Review `FrameworkStatus`, `FrameworkByteBuffer`, or result record design
- Compare current FFI coverage against `framework-system` capabilities

## Primary Artifacts

- `src/lib.rs`
- `csharp/NativeMethods.g.cs`
- `FFI_NOTES.md`
- `FRAMEWORK_DOTNET_CHANGES_TO_BE_MADE.md`
- `framework-system/framework_lib/src/power.rs`
- `framework-system/framework_lib/src/chromium_ec/mod.rs`
- `framework-system/EXAMPLES.md`

## Procedure

1. Start from the generated surface.
   - Treat `csharp/NativeMethods.g.cs` as the primary review artifact for managed ergonomics.
   - If the generated shape is awkward, change the Rust ABI instead of adding handwritten C# glue.

2. Anchor behavior in upstream Rust.
   - Read the nearest implementation in `framework-system/framework_lib/` before changing behavior.
   - Preserve upstream semantics and edge cases unless a deliberate FFI-only translation is required.

3. Shape the ABI intentionally.
   - Prefer by-value `Framework...Result` records over out parameters.
   - Keep `FrameworkStatus` as the shared tagged status field across results.
   - Prefer enums and nested structs over flat bytes or unrelated primitive fields when this improves `csbindgen` output.
   - Use `FrameworkByteBuffer` for dynamic strings or byte content the managed side should decode.
   - Preserve numeric stability when mirroring upstream enums.

4. Handle ownership explicitly.
   - If a field is exposed as `FrameworkByteBuffer`, ensure the free path is clear and documented.
   - Review nested buffers as well, not just top-level string results.

5. Validate downstream compatibility.
   - Check the changed ABI against `https://github.com/TekuSP/framework-dotnet` if the change affects generated struct layout, result records, status handling, snapshot shape, or buffer ownership.
   - Pay special attention to downstream assumptions around fixed-slot snapshots, `SensorCount` derivation, `Battery_0` and `BatteryCount`, `FrameworkByteBuffer.ToUtf8StringAndFree()`, and fan-control response wrappers.
   - If the ABI change is likely to require managed-side fixes, add or update an entry in `FRAMEWORK_DOTNET_CHANGES_TO_BE_MADE.md`.

6. Re-run feature coverage review when needed.
   - Compare exported FFI functions against `EXAMPLES.md`, README feature lists, and CLI capabilities.
   - Group missing capabilities by feature area so roadmap decisions are obvious.

7. Validate the ABI change.
   - Run `cargo build`
   - Run `cargo fmt --all -- --check`
   - Run `cargo clippy -- -D warnings`
   - Inspect the regenerated `NativeMethods.g.cs` for shape regressions

## Decision Points

- Awkward generated C# record layout:
  - Change Rust ABI shape, not handwritten C#.

- Dynamic text field or version string:
  - Prefer `FrameworkByteBuffer` and document/free ownership.

- Ambiguous byte flags:
  - Prefer explicit enums if they represent a real state machine or small closed set.

- Need `IObservable` semantics:
  - Prefer polling snapshots and building observables in C# first.
  - Add native callbacks only if polling is insufficient.

## Quality Gates

- The generated C# surface feels intentional, not mechanically flattened.
- Returned buffers have explicit ownership guidance.
- Enum values stay numerically stable across revisions.
- Build, fmt, and clippy pass.
- Downstream compatibility with `TekuSP/framework-dotnet` has been reviewed for ABI-sensitive changes.
- FFI behavior preserves upstream semantics unless a documented translation is intentional.
- Feature gap analysis is updated when scope changes materially.

## Common Outcomes

- cleaner `csbindgen` output without handwritten wrapper types
- richer error reporting through `FrameworkStatus`
- explicit enums instead of ambiguous bytes or flags
- better managed-side ergonomics with stable low-level ABI
- a concrete parity list of what is still missing from upstream capabilities
