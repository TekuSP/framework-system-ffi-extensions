---
description: "Review the generated NativeMethods.g.cs shape for the Framework System .NET FFI. Use when csbindgen output feels awkward, mechanical, redundant, or unclear and you want concrete Rust-side ABI cleanup suggestions instead of handwritten C# wrappers."
name: "FFI Generated Shape Review"
argument-hint: "What part of the generated FFI shape should be reviewed?"
agent: "agent"
---

Review the generated .NET interop shape for the current Framework System FFI and suggest improvements at the Rust ABI layer.

Use these sources as needed:

- `csharp/NativeMethods.g.cs` for the generated managed shape
- `src/lib.rs` for the Rust ABI that produced it
- `FFI_NOTES.md` for prior design decisions and learnings
- `FRAMEWORK_DOTNET_CHANGES_TO_BE_MADE.md` for downstream follow-up tracking
- `framework-system/framework_lib/src/**` when upstream behavior or data modeling matters
- `TekuSP/framework-dotnet` when downstream managed assumptions matter

Instructions:

1. Start from `csharp/NativeMethods.g.cs` as the primary review artifact.
2. Focus on generated shapes that are awkward for managed consumers, including:
   - flat bytes or flags that should be enums
   - repeated input values echoed back in result structs
   - fixed byte arrays that should be dynamic buffers
   - weakly grouped fields that should be nested records
   - ownership or free-path ambiguity
   - generated names or layouts that obscure semantics
3. For each issue, trace it back to the Rust ABI in `src/lib.rs`.
4. Output findings first, ordered by severity or value.
5. For each finding, include:
   - the problematic generated shape
   - why it is awkward in C#
   - the concrete Rust-side ABI change that would improve it
   - any compatibility or ownership tradeoff
6. When relevant, note whether `https://github.com/TekuSP/framework-dotnet` appears to rely on the current ABI shape and describe what would need to change there.
7. If a finding implies downstream follow-up, propose a concise entry that should be added to `FRAMEWORK_DOTNET_CHANGES_TO_BE_MADE.md`.
8. Prefer Rust ABI changes over handwritten C# wrappers.
9. If no meaningful issues remain, state that explicitly and note any residual risks or low-priority cleanup opportunities.
10. Do not edit files unless explicitly asked.

Keep the output concise, evidence-based, and implementation-oriented.