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

No downstream follow-up is currently recorded.

When needed, add entries in this format:

| Date | FFI change | Affected downstream area | Required framework-dotnet changes | Status |
| --- | --- | --- | --- | --- |
| YYYY-MM-DD | Brief ABI change summary | Example: thermal snapshot mapping | Example: update generated partials and managed snapshot conversion | Planned |