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

**Nothing outstanding as of 2026-08-30.** `TekuSP/framework-dotnet` consumes the full FFI surface at
`v0.6.3.98` (90 entry points): all 46 functions from the two 2026-08-30 tranches are wrapped, the
managed layer maps `FrameworkStatusCode::NotSupported` to its own exception type, and the five
bitmask enums csbindgen does not emit are mirrored by hand. The rows below are kept as history.

| Date | FFI change | Affected downstream area | Required framework-dotnet changes | Status |
| --- | --- | --- | --- | --- |
| 2026-05-18 | Added `framework_ec_get_feature_flags`, `framework_ec_get_keyboard_backlight`, `framework_ec_get_fingerprint_led`, `framework_ec_get_expansion_bay_status`, and `framework_ec_get_module_inventory` plus new module/inventory enums and records | Native method generation, managed wrappers, module inventory domain model, `FrameworkByteBuffer` handling for expansion-bay serial number | Regenerate/update interop bindings, add managed constants/helpers for the feature/module flag bitmasks, map `FrameworkEcExpansionBayStatus.serial_number` through existing byte-buffer free helpers, add wrapper/domain types for module inventory slots/descriptors, and wire the new raw readback APIs into managed services/UI | Completed (2026-08-30) |
| 2026-05-19 | Added `framework_ec_get_gpu_descriptor_header`, `framework_ec_read_gpu_descriptor`, and `framework_ec_validate_gpu_descriptor` plus new GPU descriptor header/result records | Native method generation, managed wrappers for fixed buffers and raw byte spans, `FrameworkByteBuffer` handling for descriptor blobs | Regenerate/update interop bindings, add managed helpers for reading `FrameworkGpuDescriptorHeader.magic` and `.serial` fixed buffers, add a raw descriptor wrapper that copies and frees `FrameworkEcGpuDescriptorReadResult.descriptor`, and add a validation wrapper that pins/copies caller-provided descriptor bytes before invoking the native API | Completed (2026-08-30) |
| 2026-06-23 | Replaced `FrameworkFanReading.reserved: ushort` with `FrameworkFanReading.name: FrameworkFanName`; added `FrameworkFanName` enum (`#[repr(u16)]`). Struct size and alignment are unchanged — `reserved = 0` maps to `FrameworkFanName.Unknown = 0`. | `FrameworkFanReading` layout in thermal snapshot; any code reading the old `reserved` field | Regenerate/update interop bindings to pick up `FrameworkFanName` enum and the renamed `name` field. No managed memory or ownership changes needed. Fan name can now be read directly from the thermal snapshot returned by `framework_ec_get_thermal_snapshot` — no extra call required. | Completed (2026-08-30) |
| 2026-06-23 | Added `framework_ec_get_chassis_intrusion() -> FrameworkChassisIntrusionResult`, `framework_ec_set_charge_current_limit(current_ma: u32, battery_soc: i32) -> FrameworkStatus` (battery_soc=-1 = unconditional), `framework_ec_get_uptime() -> FrameworkEcUptimeResult`, `framework_ec_get_s0ix_counter() -> FrameworkS0ixCounterResult`, `framework_ec_reset_s0ix_counter() -> FrameworkStatus`, `framework_ec_set_tablet_mode(mode: FrameworkTabletModeOverride) -> FrameworkStatus` (returns InvalidCommand on FW16/Desktop), `framework_ec_set_input_deck_mode(mode: FrameworkDeckStateMode) -> FrameworkStatus` (FW16 only). New structs: `FrameworkChassisIntrusionResult`, `FrameworkEcUptimeResult`, `FrameworkS0ixCounterResult`. New enums: `FrameworkTabletModeOverride`, `FrameworkDeckStateMode`. | Native method generation, managed wrappers for new fns/structs/enums | Regenerate/update interop bindings. Add service methods: `GetChassisIntrusion()`, `SetChargeCurrentLimit(mA, soc?)`, `GetUptime()`, `GetS0ixCounter()`, `ResetS0ixCounter()`, `SetTabletMode(mode)`, `SetInputDeckMode(mode)`. Guard tablet/deck-mode calls by platform family (`Framework12`/`Framework13` for tablet; `Framework16` for deck mode). | Completed (2026-08-30) |
| 2026-06-23 | Added `framework_ec_set_keyboard_backlight(percent: u8) -> FrameworkStatus`, `framework_ec_set_fingerprint_led(level: FrameworkFingerprintLedLevel) -> FrameworkStatus`, `framework_ec_get_privacy_switches() -> FrameworkPrivacySwitchesResult`, `framework_ec_get_charge_limits() -> FrameworkChargeLimitsResult`, `framework_ec_set_charge_limits(min, max: u8) -> FrameworkStatus`. New structs: `FrameworkPrivacySwitchesResult` (microphone_enabled, camera_enabled — 1=enabled/privacy-switch-off), `FrameworkChargeLimitsResult` (min_percent, max_percent). `set_fingerprint_led` rejects `Unknown` and `Custom` (get-only) with `InvalidArgument`. | Native method generation for 5 new fns, new managed wrappers and domain properties | (1) Regenerate/update interop bindings. (2) Expose write path for keyboard backlight (pair with existing `GetKeyboardBacklight`). (3) Expose write path for FP LED (pair with existing `GetFingerprintLed`). (4) Add `GetPrivacySwitches()` service method returning mic/camera enabled state. (5) Add `GetChargeLimits()` / `SetChargeLimits(min, max)` — highest user-facing value; enables battery health mode UX. | Completed (2026-08-30) |
| 2026-08-30 | **Additive, with one enum extension.** Second tranche: 31 more functions (59 → 90 entry points) covering the remaining `framework_lib` surface worth exposing. **`FrameworkStatusCode` gains `NotSupported = -9`** — existing values are unchanged, but managed `switch`/mapping over the status code must handle the new case or it will fall through to the default arm. New functions — PD/charger: `get_pd_controller_versions`, `get_pd_power_info(port)`, `is_charging`, `get_retimer_version`, `set_charge_rate_limit(rate_amps, battery_soc_percent)`. Thermal/power: `get_fan_count`, `get_standalone_mode`, `get_hibernate_delay`, `set_hibernate_delay`. Device: `set_fingerprint_led_percentage`, `adc_read(channel)`, `command_version_supported(command, version)`, `ps2_emulation_enable`, `remap_key(row, col, scanset)`, `remap_caps_to_ctrl`, `set_rgb_keyboard_colors(start_key, colors, color_count)`. GPIO: `get_gpio`, `set_gpio`, `get_gpio_count`, `get_gpio_info(index)`. GPU: `get_gpu_serial` (read only — the write path is deliberately not exposed). Handle-free (HID/USB direct): `framework_get_stylus_battery`, `framework_touchscreen_enable`, `framework_touchpad_set_haptic_intensity`, `framework_touchpad_set_click_force`, `framework_get_camera_versions`, `framework_get_input_module_versions`, `framework_get_usb_hub_versions`, `framework_get_audio_card_version`, `framework_get_nvme_version(path, path_length)`, `framework_peripheral_versions_free`. New enums (all emitted): `FrameworkPdFwMode`, `FrameworkPdApplication`, `FrameworkUsbPowerRole`, `FrameworkUsbChargingType`, `FrameworkClickForce`. | Native method generation; managed status-code mapping; new wrappers; buffer ownership for peripheral/GPIO/GPU-serial/retimer results | (1) Regenerate interop bindings. (2) **Handle `FrameworkStatusCode.NotSupported = -9`** in status→exception mapping: it means the capability is not compiled in for this platform (permanent), unlike `DataUnavailable` (transient read failure). Currently only `framework_get_nvme_version` returns it, on non-Linux. (3) Ten of the new functions take **no EC handle** — they talk to HID/USB directly. Expose them as static/service-level calls, not methods on an EC session. (4) Ownership: `FrameworkPeripheralVersionsResult` owns eight `product_name` buffers and must be released with `framework_peripheral_versions_free` — do **not** free them individually; `FrameworkEcGpioInfoResult.name`, `FrameworkEcGpuSerialResult.serial`, `FrameworkEcRetimerVersionResult.version` and both `FrameworkNvmeVersionResult` buffers use the plain `framework_byte_buffer_free` path. (5) String inputs (`get_gpio`, `set_gpio`, `framework_get_nvme_version`) are **pointer + length UTF-8, not NUL-terminated** — pin the bytes, pass the length. (6) `set_rgb_keyboard_colors` takes `color_count * 3` bytes in R,G,B order, max 64 keys per call. (7) `set_charge_rate_limit` uses a **negative** `battery_soc_percent` for "unconditional", matching `set_charge_current_limit`. (8) Touchpad haptic intensity and click force are **write-only** — the firmware never answers GET_FEATURE, so do not model them as round-trippable properties. (9) PD controller slots are fixed: 0 = Right01, 1 = Left23, 2 = Back; laptops populate 0 and 1, Desktop populates only 2 — always check `present`. (10) `framework_get_audio_card_version` claims the HID interface for the duration of the call and can take up to ~3s on a wedged card; keep it off the UI thread. (11) There is **no GPU serial write** — expose `get_gpu_serial` as a read-only property; do not design a managed setter around it. | Completed (2026-08-30) |
| 2026-08-30 | **Purely additive — no existing struct, enum value, or signature changed.** Submodule moved to framework-system `a338c6a` (v0.6.5-19) and 15 new functions were exported: `framework_ec_get_switches`, `framework_ec_hello(in_data)`, `framework_ec_check_hello`, `framework_ec_get_protocol_info`, `framework_ec_get_sysinfo`, `framework_ec_get_panic_info`, `framework_ec_get_port80_history`, `framework_ec_get_battery_cutoff_status`, `framework_ec_get_ap_throttle_status`, `framework_ec_get_thermal_thresholds(sensor_index)`, `framework_ec_set_thermal_thresholds(sensor_index, warn, high, halt, fan_off, fan_max)`, `framework_ec_get_temp_sensor_name(sensor_index)`, `framework_ec_get_smart_battery_data(use_unseal_key, unseal_key)`, `framework_smart_battery_data_free`, `framework_ec_authenticate_battery(auth_key)`. New structs: `FrameworkEcSwitchesResult`, `FrameworkEcHelloResult`, `FrameworkEcProtocolInfoResult`, `FrameworkEcSysinfoResult`, `FrameworkEcPanicInfoResult`, `FrameworkEcPort80HistoryResult`, `FrameworkEcBatteryCutoffResult`, `FrameworkEcApThrottleResult`, `FrameworkThermalThresholds`, `FrameworkEcThermalThresholdsResult`, `FrameworkEcTempSensorNameResult`, `FrameworkSmartBatteryData`, `FrameworkEcSmartBatteryResult`, `FrameworkEcBatteryAuthResult`. New generated enum: `FrameworkBatteryCutoffState`. Bitmask enums `FrameworkThermalThresholdFlag`, `FrameworkEcSysinfoFlag`, `FrameworkEcResetFlag`, `FrameworkEcProtocolFlag`, `FrameworkPort80Event` are **not** emitted by csbindgen (same as the existing `FrameworkEcFeatureFlag` / `FrameworkModuleFlag`) — see the bitmask constants section below. | Native method generation; new managed wrappers; buffer ownership for four new buffer-carrying results; thermal service gains a threshold read/write path | (1) Regenerate interop bindings — nothing existing needs migrating. (2) Add managed constants for the five non-emitted bitmask enums (values listed below). (3) Ownership: `FrameworkEcPanicInfoResult.data`, `FrameworkEcPort80HistoryResult.codes` and `FrameworkEcTempSensorNameResult.name` each free with the existing `framework_byte_buffer_free` / `ToUtf8StringAndFree()` helpers; `FrameworkSmartBatteryData` owns **ten** buffers and must be released with `framework_smart_battery_data_free` — do **not** free its fields individually. (4) Decode `FrameworkEcPort80HistoryResult.codes` as `history_size` little-endian `ushort` entries; newest entry is at `writes % history_size`. (5) Thermal thresholds are Celsius but a disabled threshold reads back as `-273` — always test `enabled_mask`, never the value. Setter convention: negative keeps current, `0` disables, positive is Celsius. (6) Cache `framework_ec_get_temp_sensor_name` results once per session (one host command per call) and keep polling `framework_ec_get_thermal_snapshot` for values; `mapped_name` reconciles firmware names onto the existing `FrameworkSensorName` enum. (7) `framework_ec_get_smart_battery_data` performs many I2C round trips — call on demand, never in a poll loop; `unsealed` tells you whether the SoH/safety/lifetime group is populated. (8) `framework_ec_authenticate_battery` takes a pointer to exactly **16** bytes — pin a `byte[16]` before calling; a `success` status with `authenticated = 0` means the battery answered and failed the challenge, which is not an error condition. (9) `FrameworkEcTempSensorNameResult.sensor_type` is the EC tag: 0 ignored, 1 CPU, 2 board, 3 case, 4 battery. | Completed (2026-08-30) |
| 2026-06-23 | **Breaking:** USB-C expansion card slots in `FrameworkModuleInventory` are now `FrameworkExpansionCardModuleDescriptor` (64 bytes, flat — all `FrameworkModuleDescriptor` fields inlined, plus `pd: FrameworkEcPdPortState`, `card_type`, `card_confidence`). All other slot fields (`input_top_row_N`, `input_touchpad`, `internal_*`, `expansion_bay`, `detached_N`) revert to plain `FrameworkModuleDescriptor`. Intermediate wrapper structs (`FrameworkUsbCModuleDescriptor`, `FrameworkPdModuleDescriptor`, `FrameworkInputDeckTopRowDescriptor`, `FrameworkInputDeckTouchpadDescriptor`, `FrameworkInternalModuleDescriptor`, `FrameworkExpansionBayModuleDescriptor`, `FrameworkDetachedModuleDescriptor`) are removed. Added `FrameworkModuleSlotKind.UsbCExpansionCardSlot = 7`, 7 new `FrameworkModuleIdentity` variants (22–28), `FrameworkExpansionCardType` enum, 4 PD enums (`FrameworkPdTypeCState`, `FrameworkPdPowerRole`, `FrameworkPdDataRole`, `FrameworkPdCcPolarity`), and `FrameworkEcPdPortState` struct (28 bytes). | `FrameworkModuleInventory` layout; all code reading expansion card slot fields; any switch/match on `FrameworkModuleSlotKind` or `FrameworkModuleIdentity` | (1) Regenerate/update interop bindings: remove 7 deleted wrapper structs, add flat `FrameworkExpansionCardModuleDescriptor`, add all new enums and `FrameworkEcPdPortState`. (2) Field access is now direct: `usb_c_slot_0.Identity`, `usb_c_slot_0.Flags`, `usb_c_slot_0.Pd.VoltageMv`, `usb_c_slot_0.CardType`. No `@base` chain. (3) Other slot fields (`internal_keyboard.Identity`) unchanged — still plain `FrameworkModuleDescriptor`. (4) Handle new `UsbCExpansionCardSlot` slot kind and identity variants 22–28. (5) Implement managed wrapper — see C# guidance section below. | Completed (2026-08-30) |

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

## C# Guidance: Bitmask Constants Not Emitted by csbindgen

csbindgen only emits an enum when an exported signature or struct field references it by
type. Bitmask enums are referenced only as raw `uint` / `ulong` fields, so they do not reach
`NativeMethods.g.cs` — this already applied to `FrameworkEcFeatureFlag` and
`FrameworkModuleFlag`, and now also to the five below. Mirror them in managed code.

```csharp
// FrameworkThermalThresholds.enabled_mask — a clear bit means EC firmware has that
// threshold disabled. Always test this instead of comparing the Celsius value, which
// reads back as -273 when disabled.
[Flags]
internal enum ThermalThresholdFlag : uint
{
    Warn         = 0x01,
    High         = 0x02,
    Halt         = 0x04,
    WarnRelease  = 0x08,
    HighRelease  = 0x10,
    HaltRelease  = 0x20,
    FanOff       = 0x40,
    FanMax       = 0x80,
}

// FrameworkEcSysinfoResult.flags
[Flags]
internal enum EcSysinfoFlag : uint
{
    Locked               = 0x01,  // write protect asserted, debug features disabled
    ForceLocked          = 0x02,  // locked even if write protect is deasserted
    JumpEnabled          = 0x04,
    JumpedToCurrentImage = 0x08,
    RebootAtShutdown     = 0x10,
    InManualRecovery     = 0x20,
}

// FrameworkEcSysinfoResult.reset_flags and the existing FrameworkEcUptimeResult.ec_reset_flags
[Flags]
internal enum EcResetFlag : uint
{
    Other       = 0x00000001,
    ResetPin    = 0x00000002,
    Brownout    = 0x00000004,
    PowerOn     = 0x00000008,
    Watchdog    = 0x00000010,
    Soft        = 0x00000020,
    Hibernate   = 0x00000040,
    RtcAlarm    = 0x00000080,
    WakePin     = 0x00000100,
    LowBattery  = 0x00000200,
    Sysjump     = 0x00000400,
    Hard        = 0x00000800,
    ApOff       = 0x00001000,
    Preserved   = 0x00002000,
    UsbResume   = 0x00004000,
    Rdd         = 0x00008000,
    Rbox        = 0x00010000,
    Security    = 0x00020000,
    ApWatchdog  = 0x00040000,
    StayInRo    = 0x00080000,
    Efs         = 0x00100000,
    ApIdle      = 0x00200000,
    InitialPwr  = 0x00400000,
}

// FrameworkEcProtocolInfoResult.flags
[Flags]
internal enum EcProtocolFlag : uint
{
    InProgressSupported = 0x01,
}

// Marker codes the EC inserts into the port 80 history buffer.
internal enum Port80Event : ushort
{
    Resume = 0x1001,
    Reset  = 0x1002,
}
```

`FrameworkEcProtocolInfoResult.protocol_versions` is a plain version bitmask, not one of the
above: bit N set means protocol version N is supported.

---

When needed, add entries in this format:

| Date | FFI change | Affected downstream area | Required framework-dotnet changes | Status |
| --- | --- | --- | --- | --- |
| YYYY-MM-DD | Brief ABI change summary | Example: thermal snapshot mapping | Example: update generated partials and managed snapshot conversion | Planned |
