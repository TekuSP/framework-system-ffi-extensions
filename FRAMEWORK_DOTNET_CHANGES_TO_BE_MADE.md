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
| 2026-06-23 | Added `framework_ec_get_chassis_intrusion() -> FrameworkChassisIntrusionResult`, `framework_ec_set_charge_current_limit(current_ma: u32, battery_soc: i32) -> FrameworkStatus` (battery_soc=-1 = unconditional), `framework_ec_get_uptime() -> FrameworkEcUptimeResult`, `framework_ec_get_s0ix_counter() -> FrameworkS0ixCounterResult`, `framework_ec_reset_s0ix_counter() -> FrameworkStatus`, `framework_ec_set_tablet_mode(mode: FrameworkTabletModeOverride) -> FrameworkStatus` (returns InvalidCommand on FW16/Desktop), `framework_ec_set_input_deck_mode(mode: FrameworkDeckStateMode) -> FrameworkStatus` (FW16 only). New structs: `FrameworkChassisIntrusionResult`, `FrameworkEcUptimeResult`, `FrameworkS0ixCounterResult`. New enums: `FrameworkTabletModeOverride`, `FrameworkDeckStateMode`. | Native method generation, managed wrappers for new fns/structs/enums | Regenerate/update interop bindings. Add service methods: `GetChassisIntrusion()`, `SetChargeCurrentLimit(mA, soc?)`, `GetUptime()`, `GetS0ixCounter()`, `ResetS0ixCounter()`, `SetTabletMode(mode)`, `SetInputDeckMode(mode)`. Guard tablet/deck-mode calls by platform family (`Framework12`/`Framework13` for tablet; `Framework16` for deck mode). | Planned |
| 2026-06-23 | Added `framework_ec_set_keyboard_backlight(percent: u8) -> FrameworkStatus`, `framework_ec_set_fingerprint_led(level: FrameworkFingerprintLedLevel) -> FrameworkStatus`, `framework_ec_get_privacy_switches() -> FrameworkPrivacySwitchesResult`, `framework_ec_get_charge_limits() -> FrameworkChargeLimitsResult`, `framework_ec_set_charge_limits(min, max: u8) -> FrameworkStatus`. New structs: `FrameworkPrivacySwitchesResult` (microphone_enabled, camera_enabled — 1=enabled/privacy-switch-off), `FrameworkChargeLimitsResult` (min_percent, max_percent). `set_fingerprint_led` rejects `Unknown` and `Custom` (get-only) with `InvalidArgument`. | Native method generation for 5 new fns, new managed wrappers and domain properties | (1) Regenerate/update interop bindings. (2) Expose write path for keyboard backlight (pair with existing `GetKeyboardBacklight`). (3) Expose write path for FP LED (pair with existing `GetFingerprintLed`). (4) Add `GetPrivacySwitches()` service method returning mic/camera enabled state. (5) Add `GetChargeLimits()` / `SetChargeLimits(min, max)` — highest user-facing value; enables battery health mode UX. | Planned |
| 2026-06-23 | **Breaking:** USB-C expansion card slots in `FrameworkModuleInventory` are now `FrameworkExpansionCardModuleDescriptor` (64 bytes, flat — all `FrameworkModuleDescriptor` fields inlined, plus `pd: FrameworkEcPdPortState`, `card_type`, `card_confidence`). All other slot fields (`input_top_row_N`, `input_touchpad`, `internal_*`, `expansion_bay`, `detached_N`) revert to plain `FrameworkModuleDescriptor`. Intermediate wrapper structs (`FrameworkUsbCModuleDescriptor`, `FrameworkPdModuleDescriptor`, `FrameworkInputDeckTopRowDescriptor`, `FrameworkInputDeckTouchpadDescriptor`, `FrameworkInternalModuleDescriptor`, `FrameworkExpansionBayModuleDescriptor`, `FrameworkDetachedModuleDescriptor`) are removed. Added `FrameworkModuleSlotKind.UsbCExpansionCardSlot = 7`, 7 new `FrameworkModuleIdentity` variants (22–28), `FrameworkExpansionCardType` enum, 4 PD enums (`FrameworkPdTypeCState`, `FrameworkPdPowerRole`, `FrameworkPdDataRole`, `FrameworkPdCcPolarity`), and `FrameworkEcPdPortState` struct (28 bytes). | `FrameworkModuleInventory` layout; all code reading expansion card slot fields; any switch/match on `FrameworkModuleSlotKind` or `FrameworkModuleIdentity` | (1) Regenerate/update interop bindings: remove 7 deleted wrapper structs, add flat `FrameworkExpansionCardModuleDescriptor`, add all new enums and `FrameworkEcPdPortState`. (2) Field access is now direct: `usb_c_slot_0.Identity`, `usb_c_slot_0.Flags`, `usb_c_slot_0.Pd.VoltageMv`, `usb_c_slot_0.CardType`. No `@base` chain. (3) Other slot fields (`internal_keyboard.Identity`) unchanged — still plain `FrameworkModuleDescriptor`. (4) Handle new `UsbCExpansionCardSlot` slot kind and identity variants 22–28. (5) Implement managed wrapper — see C# guidance section below. | Planned |

## C# Guidance: Expansion Card Module Design

`FrameworkExpansionCardModuleDescriptor` is a **flat FFI struct** — all fields are direct members,
no `@base` navigation. The .NET best practice for FFI structs is independent flat types; the
`slot_kind` field on `FrameworkModuleDescriptor` carries the semantic type tag for all other slots.

```csharp
// Expansion card type hierarchy — one class per FrameworkExpansionCardType variant.
// typeof/is work on these managed types, not on FFI structs.
public abstract class FrameworkExpansionCard { }
public sealed class DisplayPortCard  : FrameworkExpansionCard { }
public sealed class HdmiCard         : FrameworkExpansionCard { }
public sealed class AudioCard        : FrameworkExpansionCard { }
public sealed class UsbACard         : FrameworkExpansionCard { }
public sealed class UsbCCard         : FrameworkExpansionCard { }
public sealed class EthernetCard     : FrameworkExpansionCard { }   // 2.5G RTL8156B
public sealed class Ethernet10GCard  : FrameworkExpansionCard { }   // 10G WisdPi
public sealed class MicroSdCard      : FrameworkExpansionCard { }
public sealed class SdCard           : FrameworkExpansionCard { }   // full-size SD
public sealed class SsdCard          : FrameworkExpansionCard { }   // NVMe storage
public sealed class UnknownCard      : FrameworkExpansionCard { }

// Managed record wrapping the flat FFI struct
public sealed record ExpansionCardSlot(FrameworkExpansionCardModuleDescriptor Raw)
{
    public FrameworkModuleIdentity   Identity   => Raw.identity;
    public FrameworkEcPdPortState    Pd         => Raw.pd;
    public FrameworkExpansionCardType CardType  => Raw.card_type;
    public FrameworkModuleConfidence Confidence => Raw.card_confidence;
    public bool IsPresent => Raw.present != 0;
}

// Factory — produces typed slot + typed card
public static ExpansionCardSlot FromDescriptor(FrameworkExpansionCardModuleDescriptor d)
{
    var slot = new ExpansionCardSlot(d);
    // use slot.CardType to create the typed FrameworkExpansionCard subclass
    return slot;
}
```

Field access from `FrameworkModuleInventory` — all direct, no chain:

- `inventory.usb_c_slot_0.identity` — module identity
- `inventory.usb_c_slot_0.flags` — flags (use `FrameworkModuleFlag` constants)
- `inventory.usb_c_slot_0.pd.voltage_mv` — negotiated voltage
- `inventory.usb_c_slot_0.card_type` — typed card discriminant
- `inventory.internal_keyboard.identity` — plain `FrameworkModuleDescriptor`, direct access
- `inventory.expansion_bay.identity` — plain `FrameworkModuleDescriptor`, `slot_kind = ExpansionBay`

---

When needed, add entries in this format:

| Date | FFI change | Affected downstream area | Required framework-dotnet changes | Status |
| --- | --- | --- | --- | --- |
| YYYY-MM-DD | Brief ABI change summary | Example: thermal snapshot mapping | Example: update generated partials and managed snapshot conversion | Planned |
