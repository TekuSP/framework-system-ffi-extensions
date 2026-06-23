# FrameworkDotnet Changes To Be Made

This document tracks follow-up work required in `https://github.com/TekuSP/framework-dotnet`
when the Rust FFI in this repository changes in a way that may break existing managed
logic, generated helpers, exception mapping, or snapshot consumption patterns.

## How To Use

- Review `TekuSP/framework-dotnet` when making ABI-sensitive changes here.
- If the change is likely to require managed-side updates, add or update an entry below
  before considering the FFI change complete.
- Remove or mark entries complete once downstream changes have been made.

## Downstream Assumptions To Watch

The managed repo currently appears to rely on several important ABI conventions:

- fixed-slot snapshot layout for thermal and power data
- `FanCount` plus managed `SensorCount` derivation for thermal snapshots
- `Battery_0` and `BatteryCount` for power snapshots
- `FrameworkByteBuffer` helpers such as `ToUtf8StringAndFree()` for strings and version fields
- `FrameworkStatus` and result records being translated into managed exceptions
- fan-control result records carrying enough data to construct managed response objects

These assumptions do not mean the ABI cannot change, but they do mean shape changes
should be reviewed explicitly rather than treated as internal-only refactors.

## Pending Changes

| Date | FFI change | Affected downstream area | Required framework-dotnet changes | Status |
| --- | --- | --- | --- | --- |
| 2026-05-18 | Added `framework_ec_get_feature_flags`, `framework_ec_get_keyboard_backlight`, `framework_ec_get_fingerprint_led`, `framework_ec_get_expansion_bay_status`, and `framework_ec_get_module_inventory` plus new module/inventory enums and records | Native method generation, managed wrappers, module inventory domain model, `FrameworkByteBuffer` handling for expansion-bay serial number | Regenerate/update interop bindings, add managed constants/helpers for the feature/module flag bitmasks, map `FrameworkEcExpansionBayStatus.serial_number` through existing byte-buffer free helpers, add wrapper/domain types for module inventory slots/descriptors, and wire the new raw readback APIs into managed services/UI | Planned |
| 2026-05-19 | Added `framework_ec_get_gpu_descriptor_header`, `framework_ec_read_gpu_descriptor`, and `framework_ec_validate_gpu_descriptor` plus new GPU descriptor header/result records | Native method generation, managed wrappers for fixed buffers and raw byte spans, `FrameworkByteBuffer` handling for descriptor blobs | Regenerate/update interop bindings, add managed helpers for reading `FrameworkGpuDescriptorHeader.magic` and `.serial` fixed buffers, add a raw descriptor wrapper that copies and frees `FrameworkEcGpuDescriptorReadResult.descriptor`, and add a validation wrapper that pins/copies caller-provided descriptor bytes before invoking the native API | Planned |
| 2026-06-23 | Replaced `FrameworkFanReading.reserved: ushort` with `FrameworkFanReading.name: FrameworkFanName`; added `FrameworkFanName` enum (`#[repr(u16)]`). Struct size and alignment are unchanged — `reserved = 0` maps to `FrameworkFanName.Unknown = 0`. | `FrameworkFanReading` layout in thermal snapshot; any code reading the old `reserved` field | Regenerate/update interop bindings to pick up `FrameworkFanName` enum and the renamed `name` field. No managed memory or ownership changes needed. Fan name can now be read directly from the thermal snapshot returned by `framework_ec_get_thermal_snapshot` — no extra call required. | Planned |

When needed, add entries in this format:

| Date | FFI change | Affected downstream area | Required framework-dotnet changes | Status |
| --- | --- | --- | --- | --- |
| YYYY-MM-DD | Brief ABI change summary | Example: thermal snapshot mapping | Example: update generated partials and managed snapshot conversion | Planned |
