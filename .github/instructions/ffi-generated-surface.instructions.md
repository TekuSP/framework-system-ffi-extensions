---
description: "Use when editing src/lib.rs or changing the Rust FFI ABI for csbindgen. After any exported ABI change, inspect csharp/NativeMethods.g.cs, review the generated shape, and validate build, fmt, and clippy before considering the change complete."
name: "FFI Generated Surface Review"
applyTo: "src/lib.rs"
---

# FFI Generated Surface Review

- Treat `csharp/NativeMethods.g.cs` as the primary review artifact for managed ergonomics.
- After changing any `extern "C"` function, `#[repr(C)]` struct, `#[repr(...)]` enum, union, or ownership-relevant field in `src/lib.rs`, inspect the regenerated C# output before making more ABI edits.
- Prefer fixing awkward generated shapes in the Rust ABI instead of adding handwritten C# wrappers.
- Validate ABI-sensitive changes against downstream usage in `https://github.com/TekuSP/framework-dotnet` so existing managed assumptions are not broken silently.
- Pay special attention to fixed-slot snapshot shapes, `FrameworkByteBuffer` ownership helpers, `FrameworkStatus`-driven exception flow, and fan-control response shapes that downstream code may already map into managed responses.
- If an ABI change is likely to require updates in `TekuSP/framework-dotnet`, record the required managed-side follow-up in `FRAMEWORK_DOTNET_CHANGES_TO_BE_MADE.md`.
- Check whether new or changed `FrameworkByteBuffer` fields need explicit ownership notes or free-path updates.
- Preserve numeric stability for mirrored enums when upstream adds variants.
- Validate with:
  - `cargo build`
  - `cargo fmt --all -- --check`
  - `cargo clippy -- -D warnings`
