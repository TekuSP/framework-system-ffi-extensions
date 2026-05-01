---
description: "Generate an FFI parity table for the Framework System .NET facade. Use when comparing current FFI exports against EXAMPLES.md, README feature lists, or CLI capabilities to see what is exposed, missing, or partially covered."
name: "FFI Parity Table"
argument-hint: "Which feature scope should the parity table cover?"
agent: "agent"
---

Generate a parity table for the current Framework System .NET FFI in this repository.

Use these sources as needed:

- `src/lib.rs` for exported FFI surface and ABI types
- `csharp/NativeMethods.g.cs` for generated managed shape
- `framework-system/EXAMPLES.md` for user-facing capabilities
- `framework-system/README.md` for feature summaries
- `framework-system/framework_lib/src/commandline/mod.rs` when CLI coverage details matter
- `FFI_NOTES.md` for previously identified gaps and priorities
- `TekuSP/framework-dotnet` when checking whether current FFI shape is already consumed by downstream managed code

Instructions:

1. Determine the current exported FFI capability set.
2. Compare it against the requested scope, or all major feature areas if no scope is provided.
3. Group results by feature area.
4. Produce a markdown table with these columns:
   - Feature area
   - Upstream capability
   - FFI status
   - Notes
5. Use `FFI status` values such as `Exposed`, `Partial`, `Missing`, or `Not worth exposing`.
6. If downstream compatibility with `TekuSP/framework-dotnet` is relevant, add a short `Downstream compatibility notes` section calling out any currently relied-on shapes or likely breakpoints.
7. After the table, add a short `Highest-value next additions` section with the most important missing capabilities for the requested scope.
8. Do not edit files unless explicitly asked.


Favor concrete, evidence-based comparison over exhaustive prose.