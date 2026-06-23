# NativeMethods ABI Types Reference

This document explains the enums, structs, and related ABI shapes used by the generated interop layer in `csharp/NativeMethods.g.cs`.

The source of truth for these layouts is `src/lib.rs`. A few Rust-only flag enums are also included here because they explain numeric bitfields exposed through the generated C# types.

## Conventions

- Most result structs start with a `FrameworkStatus status` field.
  - `status.code == Success` means the payload is valid.
  - Non-success codes usually mean the payload is defaulted, partial, or should be treated as unavailable.
- Fields named `reserved`, `reserved_0`, etc. are padding / forward-compatibility fields.
  - Read them as opaque.
  - Write them as zero if you ever construct these types manually.
- Several bool-like values are represented as `byte` instead of C# `bool`.
  - `0` = false
  - non-zero = true
- Fixed byte arrays such as `magic[4]` and `serial[20]` are raw bytes, not managed strings.
- `FrameworkByteBuffer` owns native memory.
  - Buffers returned by the library must be released with `framework_byte_buffer_free`.
- `FrameworkEcHandle` is an opaque native handle.
  - Create it with `framework_ec_open_*`.
  - Release it with `framework_ec_close`.
- Two Rust flag enums are important even though the current generated C# file does not emit them directly:
  - `FrameworkEcFeatureFlag` explains `FrameworkEcFeatureFlagsResult.flags`
  - `FrameworkModuleFlag` explains `FrameworkModuleDescriptor.flags`

## Status and error-reporting types

### `FrameworkStatusCode`

Top-level success / failure code used throughout the ABI.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Success` | `0` | Operation succeeded. |
| `NullPointer` | `-1` | A required pointer argument was null. |
| `InvalidArgument` | `-2` | An argument was malformed, out of range, or otherwise invalid. |
| `NoDriverAvailable` | `-3` | No usable EC driver/backend could be opened. |
| `UnsupportedDriver` | `-4` | The selected driver/backend is not supported in the current environment. |
| `DeviceError` | `-5` | A backend or device-specific error occurred. See payload for details. |
| `EcResponse` | `-6` | The EC returned a known failure response code. |
| `UnknownResponseCode` | `-7` | The EC returned a response code the wrapper does not recognize. |
| `DataUnavailable` | `-8` | The call succeeded structurally, but the requested data is not available on this platform/device/state. |

### `FrameworkStatusNoPayload`

Payload record used when a `FrameworkStatus` has no extra detail.

| Field | Type | Meaning |
| --- | --- | --- |
| `reserved` | `int` | Unused filler field. Ignore it. |

### `FrameworkStatusInvalidFanIndexRecord`

Payload record for invalid fan index errors.

| Field | Type | Meaning |
| --- | --- | --- |
| `fan_index` | `int` | The fan index the caller supplied that could not be accepted. |

### `FrameworkStatusEcResponseRecord`

Payload record for known EC response failures.

| Field | Type | Meaning |
| --- | --- | --- |
| `response` | `FrameworkEcResponseDetail` | The decoded EC response code. |

### `FrameworkStatusUnknownEcResponseCodeRecord`

Payload record for unrecognized EC response values.

| Field | Type | Meaning |
| --- | --- | --- |
| `response_code` | `int` | Raw EC response code returned by firmware. |

### `FrameworkStatusDeviceErrorRecord`

Payload record for device/backend errors that have a stored message token.

| Field | Type | Meaning |
| --- | --- | --- |
| `message_token` | `int` | Token used by `framework_status_get_device_error_message` to recover a human-readable message. |

### `FrameworkStatusPayload` (union)

Tagged union carrying extra detail for `FrameworkStatus`.

> Only one interpretation is valid at a time. Pick the active field based on `FrameworkStatus.code`.

| Field | Type | Meaning |
| --- | --- | --- |
| `none` | `FrameworkStatusNoPayload` | Used when there is no detailed payload. |
| `invalid_fan_index` | `FrameworkStatusInvalidFanIndexRecord` | Used with invalid fan index failures. |
| `ec_response` | `FrameworkStatusEcResponseRecord` | Used when the EC returned a known failure code. |
| `unknown_ec_response_code` | `FrameworkStatusUnknownEcResponseCodeRecord` | Used when the EC returned an unknown code. |
| `device_error` | `FrameworkStatusDeviceErrorRecord` | Used for device/backend errors with a stored message token. |

### `FrameworkStatus`

Standard status object embedded in nearly every result struct.

| Field | Type | Meaning |
| --- | --- | --- |
| `code` | `FrameworkStatusCode` | Overall success/failure category. |
| `payload` | `FrameworkStatusPayload` | Optional structured detail matching the code. |

### `FrameworkEcResponseDetail`

Decoded EC response codes.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `-1` | Wrapper could not map the response cleanly. |
| `Success` | `0` | EC command succeeded. |
| `InvalidCommand` | `1` | EC did not recognize the command. |
| `Error` | `2` | Generic EC-side failure. |
| `InvalidParameter` | `3` | EC rejected one or more parameters. |
| `AccessDenied` | `4` | Command not allowed in current mode/state. |
| `InvalidResponse` | `5` | Response payload was malformed or unexpected. |
| `InvalidVersion` | `6` | Command version not supported. |
| `InvalidChecksum` | `7` | Request/response checksum failed. |
| `InProgress` | `8` | Command is still in progress. |
| `Unavailable` | `9` | Resource/feature is currently unavailable. |
| `Timeout` | `10` | Request timed out. |
| `Overflow` | `11` | Buffer or value overflow condition. |
| `InvalidHeader` | `12` | Header/layout error in request or response. |
| `RequestTruncated` | `13` | Request was shorter than expected. |
| `ResponseTooBig` | `14` | Response exceeded allowed size. |
| `BusError` | `15` | Underlying transport/bus error. |
| `Busy` | `16` | EC is busy and could not process the request immediately. |

### `FrameworkStatusDeviceErrorMessageResult`

Returns a human-readable message for a status carrying a device-error token.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Status of the message lookup itself. |
| `message` | `FrameworkByteBuffer` | UTF-8 message bytes. Free with `framework_byte_buffer_free`. |

### `FrameworkStatusDescriptionResult`

Returns a generic human-readable description for any `FrameworkStatus`.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Status of the description lookup itself. |
| `description` | `FrameworkByteBuffer` | UTF-8 description bytes. Free with `framework_byte_buffer_free`. |

## Handle, driver, platform, and product types

### `FrameworkEcHandle`

Opaque native EC handle.

| Field | Type | Meaning |
| --- | --- | --- |
| _(none exposed in generated C#)_ | — | Treat as an opaque token only. Do not inspect or construct manually. |

### `FrameworkEcDriver`

Backend used to talk to the Embedded Controller.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `-1` | No known backend selected. |
| `Portio` | `0` | Port I/O backend. |
| `CrosEc` | `1` | ChromeOS EC/backend path. |
| `Windows` | `2` | Windows-specific backend. |

### `FrameworkPlatform`

Detected machine/platform identifier.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Framework12IntelGen13` | `0` | Framework Laptop 12 with Intel Gen 13 class platform. |
| `IntelGen11` | `1` | Framework 13 Intel Gen 11 platform. |
| `IntelGen12` | `2` | Framework 13 Intel Gen 12 platform. |
| `IntelGen13` | `3` | Framework 13 Intel Gen 13 platform. |
| `IntelCoreUltra1` | `4` | Framework 13 Intel Core Ultra (first supported generation). |
| `Framework13Amd7080` | `5` | Framework 13 AMD 7040/7080 series platform. |
| `Framework13AmdAi300` | `6` | Framework 13 AMD AI 300 series platform. |
| `Framework16Amd7080` | `7` | Framework 16 AMD 7040/7080 series platform. |
| `Framework16AmdAi300` | `8` | Framework 16 AMD AI 300 series platform. |
| `FrameworkDesktopAmdAiMax300` | `9` | Framework Desktop AMD AI Max 300 platform. |
| `GenericFramework` | `10` | Framework-branded system detected, but not mapped to a more specific platform. |
| `UnknownSystem` | `11` | Platform could not be identified. |
| `IntelCoreUltra3` | `12` | Framework 13 Intel Core Ultra 3 platform. |

### `FrameworkPlatformFamily`

Higher-level family grouping.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `-1` | Family could not be determined. |
| `Framework12` | `0` | Framework 12 family. |
| `Framework13` | `1` | Framework 13 family. |
| `Framework16` | `2` | Framework 16 family. |
| `FrameworkDesktop` | `3` | Framework Desktop family. |

### `FrameworkEcHandleResult`

Result of opening an EC handle.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the handle open succeeded. |
| `handle` | `FrameworkEcHandle*` | Opaque handle on success; close with `framework_ec_close`. |

### `FrameworkPlatformResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether platform detection succeeded. |
| `platform` | `FrameworkPlatform` | Detected platform value. |

### `FrameworkPlatformFamilyResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether family detection succeeded. |
| `family` | `FrameworkPlatformFamily` | Detected family value. |

### `FrameworkEcActiveDriverResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the query succeeded. |
| `driver` | `FrameworkEcDriver` | Driver/backend associated with the handle. |

### `FrameworkProductNameResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether product-name lookup succeeded. |
| `product_name` | `FrameworkByteBuffer` | UTF-8 product name bytes. Free with `framework_byte_buffer_free`. |

### `FrameworkEcBuildInfoResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether build-info lookup succeeded. |
| `build_info` | `FrameworkByteBuffer` | UTF-8 build/version string. Free with `framework_byte_buffer_free`. |

## Thermal, fan, power, flash, and buffer types

### `FrameworkTemperatureState`

Status of an individual temperature sensor reading.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Ok` | `0` | Sensor reading is valid. |
| `NotPresent` | `1` | Sensor/channel does not exist. |
| `Error` | `2` | Sensor/channel reported an error. |
| `NotPowered` | `3` | Sensor exists but is not currently powered/available. |
| `NotCalibrated` | `4` | Sensor exists but calibration/state prevents a valid reading. |

### `FrameworkEcCurrentImage`

Which EC firmware image is currently active.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `0` | Current image could not be determined. |
| `Ro` | `1` | Read-only EC image is active. |
| `Rw` | `2` | Read-write EC image is active. |

### `FrameworkTemperatureReading`

One temperature channel snapshot.

| Field | Type | Meaning |
| --- | --- | --- |
| `state` | `FrameworkTemperatureState` | Validity/state of the reading. |
| `celsius` | `short` | Temperature value in degrees Celsius when `state == Ok`. |
| `reserved` | `ushort` | Padding / future use. |

### `FrameworkFanState`

Status of an individual fan reading.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Ok` | `0` | Fan reading is valid. |
| `NotPresent` | `1` | Fan/channel does not exist. |
| `Stalled` | `2` | Fan exists but appears stalled. |

### `FrameworkFanName`

Platform-specific role name for a fan slot.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `0` | Platform family could not be determined; slot role is indeterminate. |
| `Generic` | `1` | Family known but no specific name assigned to this slot (shown as "Fan N" in CLI). |
| `ApuFan` | `2` | Framework 12, 13, or Desktop first fan. |
| `LeftFan` | `3` | Framework 16 left fan (slot 0). |
| `RightFan` | `4` | Framework 16 right fan (slot 1). |
| `FrontFan` | `5` | Framework Desktop front fan (slot 1). |
| `ThirdFan` | `6` | Framework Desktop third fan (slot 2). |

### `FrameworkFanFeaturesState`

Feature bitset-like enum describing available fan support.

| Name | Value | Meaning |
| --- | ---: | --- |
| `None` | `0` | No supported fan-related feature detected. |
| `FanControl` | `1` | Manual fan control is supported. |
| `ThermalReporting` | `2` | Fan/thermal reporting is supported. |
| `All` | `3` | Both manual control and reporting are supported. |

### `FrameworkFanReading`

One fan channel snapshot.

| Field | Type | Meaning |
| --- | --- | --- |
| `state` | `FrameworkFanState` | Validity/state of the fan reading. |
| `rpm` | `ushort` | Fan speed in RPM when `state == Ok`. |
| `name` | `FrameworkFanName` | Platform-specific role name for this fan slot. |

### `FrameworkThermalSnapshot`

Fixed-size thermal report containing up to 8 temperature channels and up to 4 fan channels.

| Field | Type | Meaning |
| --- | --- | --- |
| `fan_count` | `byte` | Number of meaningful fan entries in `fan_0` ... `fan_3`. |
| `reserved` | `byte[3]` | Padding / future use. |
| `temperature_0` ... `temperature_7` | `FrameworkTemperatureReading` | Up to eight temperature channels. Unused entries are defaulted. |
| `fan_0` ... `fan_3` | `FrameworkFanReading` | Up to four fan channels. Unused entries are defaulted. |

### `FrameworkFanCapabilities`

Compact description of supported fan functionality.

| Field | Type | Meaning |
| --- | --- | --- |
| `fan_count` | `byte` | Number of supported fan channels. |
| `features` | `FrameworkFanFeaturesState` | Which fan-control/reporting capabilities are available. |
| `reserved` | `byte[2]` | Padding / future use. |

### `FrameworkPowerSourceState`

Overall machine power-source state.

| Name | Value | Meaning |
| --- | ---: | --- |
| `None` | `0` | No usable power-source information is available. |
| `AcOnly` | `1` | External power is present and no battery is reported. |
| `BatteryOnly` | `2` | Running on battery power. |
| `AcAndBattery` | `3` | AC is present and a battery is present. |

### `FrameworkBatteryState`

High-level battery state.

| Name | Value | Meaning |
| --- | ---: | --- |
| `NotPresent` | `0` | No battery is present. |
| `Idle` | `1` | Battery is present but neither clearly charging nor discharging. |
| `Charging` | `2` | Battery is charging. |
| `Discharging` | `3` | Battery is discharging. |
| `ChargingAndDischarging` | `4` | Firmware reports overlapping charging/discharging state. |
| `Critical` | `5` | Battery is in a critical/low state. |

### `FrameworkBatterySnapshot`

Battery metrics for the first exposed battery slot.

| Field | Type | Meaning |
| --- | --- | --- |
| `battery_state` | `FrameworkBatteryState` | High-level state of the battery. |
| `reserved` | `byte[3]` | Padding / future use. |
| `present_voltage` | `uint` | Firmware-reported present voltage value. |
| `present_rate` | `uint` | Firmware-reported current rate value. |
| `remaining_capacity` | `uint` | Firmware-reported remaining capacity. |
| `design_capacity` | `uint` | Firmware-reported design capacity. |
| `design_voltage` | `uint` | Firmware-reported design voltage. |
| `last_full_charge_capacity` | `uint` | Firmware-reported last full charge capacity. |
| `cycle_count` | `uint` | Reported battery cycle count. |
| `charge_percentage` | `uint` | Reported charge percentage. |
| `manufacturer` | `FrameworkByteBuffer` | UTF-8 manufacturer string. Free with `framework_byte_buffer_free`. |
| `model_number` | `FrameworkByteBuffer` | UTF-8 model string. Free with `framework_byte_buffer_free`. |
| `serial_number` | `FrameworkByteBuffer` | UTF-8 serial string. Free with `framework_byte_buffer_free`. |
| `battery_type` | `FrameworkByteBuffer` | UTF-8 chemistry/type string. Free with `framework_byte_buffer_free`. |

### `FrameworkPowerSnapshot`

Top-level power snapshot.

| Field | Type | Meaning |
| --- | --- | --- |
| `power_source_state` | `FrameworkPowerSourceState` | High-level power-source summary. |
| `battery_count` | `byte` | Number of batteries detected. The current ABI exposes details for the first battery in `battery_0`. |
| `reserved` | `byte[2]` | Padding / future use. |
| `battery_0` | `FrameworkBatterySnapshot` | Snapshot for the first battery slot. |

### `FrameworkEcFlashVersions`

EC flash-image version strings.

| Field | Type | Meaning |
| --- | --- | --- |
| `current_image` | `FrameworkEcCurrentImage` | Which EC image is active right now. |
| `ro_version` | `FrameworkByteBuffer` | UTF-8 RO image version string. Free with `framework_byte_buffer_free`. |
| `rw_version` | `FrameworkByteBuffer` | UTF-8 RW image version string. Free with `framework_byte_buffer_free`. |

### `FrameworkByteBuffer`

Owned native byte buffer used for strings and arbitrary blobs.

| Field | Type | Meaning |
| --- | --- | --- |
| `ptr` | `byte*` | Pointer to the first byte of the native buffer. |
| `length` | `int` | Number of meaningful bytes. |
| `capacity` | `int` | Allocated byte capacity of the native buffer. |

### `FrameworkEcFlashVersionsResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the query succeeded. |
| `versions` | `FrameworkEcFlashVersions` | Returned flash version data. |

### `FrameworkEcPowerSnapshotResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the snapshot succeeded. |
| `snapshot` | `FrameworkPowerSnapshot` | Power snapshot payload. |

### `FrameworkEcFanCapabilitiesResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the query succeeded. |
| `capabilities` | `FrameworkFanCapabilities` | Supported fan capabilities. |

### `FrameworkEcThermalSnapshotResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the snapshot succeeded. |
| `snapshot` | `FrameworkThermalSnapshot` | Thermal/fan snapshot payload. |

### `FrameworkEcSetFanRpmResult`

Echo/result for manual fan RPM requests.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the operation succeeded. |
| `fan_index` | `int` | Caller-requested fan index (echoed back). |
| `rpm` | `uint` | Caller-requested target RPM (echoed back). |

### `FrameworkEcSetFanDutyResult`

Echo/result for manual fan duty requests.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the operation succeeded. |
| `fan_index` | `int` | Caller-requested fan index (echoed back). |
| `percent` | `uint` | Caller-requested duty percentage (echoed back). |

### `FrameworkEcRestoreAutoFanControlResult`

Echo/result for restoring automatic fan control.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the operation succeeded. |
| `fan_index` | `int` | Caller-requested fan index (echoed back). |

## Feature flags, fingerprint, expansion bay, GPU descriptor, and inventory types

### `FrameworkEcFeatureFlag` (Rust-side bitflags)

These bits explain the `ulong flags` value in `FrameworkEcFeatureFlagsResult`.

| Name | Value | Meaning |
| --- | ---: | --- |
| `None` | `0` | No known optional EC features. |
| `Keyboard` | `1 << 0` | Keyboard-related EC support is available. |
| `KeyboardBacklight` | `1 << 1` | Keyboard backlight queries are supported. |
| `Touchpad` | `1 << 2` | Touchpad-related support is available. |
| `Fingerprint` | `1 << 3` | Fingerprint reader / fingerprint LED support is available. |
| `AmbientLight` | `1 << 4` | Ambient-light functionality is available. |
| `TabletMode` | `1 << 5` | Tablet-mode reporting/support is available. |

### `FrameworkFingerprintLedLevel`

Interpreted fingerprint LED intensity/mode.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `-1` | Raw level could not be mapped. |
| `High` | `0` | High brightness. |
| `Medium` | `1` | Medium brightness. |
| `Low` | `2` | Low brightness. |
| `UltraLow` | `3` | Very low brightness. |
| `Custom` | `0xFE` | Firmware reports a custom/nonstandard level. |
| `Auto` | `0xFF` | Firmware-managed automatic mode. |

### `FrameworkExpansionBayBoard`

Physical expansion-bay board type or board-status classification.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `0` | Board type could not be identified. |
| `DualInterposer` | `1` | Dual interposer board. |
| `SingleInterposer` | `2` | Single interposer board. |
| `UmaFans` | `3` | UMA/fan board configuration. |
| `NoModule` | `4` | Bay reports that no module is installed. |
| `BadConnection` | `5` | A module or board may be present but connection/state is faulty. |

### `FrameworkExpansionBayVendor`

Occupant/vendor family currently associated with the expansion bay.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `0` | Vendor/family could not be identified. |
| `Initializing` | `1` | Bay/module is still initializing. |
| `FanOnly` | `2` | Bay is populated only by a fan assembly. |
| `SsdHolder` | `3` | SSD holder/storage module. |
| `PcieAccessory` | `4` | Generic PCIe accessory module. |
| `AmdGpu` | `5` | AMD GPU module. |
| `NvidiaGpu` | `6` | NVIDIA GPU module. |

### `FrameworkGpuPcieConfig`

PCIe lane/generation configuration reported for the bay GPU/accessory path.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `0` | PCIe configuration could not be determined. |
| `Pcie4x1` | `1` | PCIe Gen 4 x1. |
| `Pcie4x2` | `2` | PCIe Gen 4 x2. |
| `Pcie4x4` | `3` | PCIe Gen 4 x4. |
| `Pcie5x4` | `4` | PCIe Gen 5 x4. |

### `FrameworkModuleIdentity`

Best-effort classification for a detected module or slot occupant.

| Name | Value | Meaning |
| --- | ---: | --- |
| `None` | `0` | No module / no identity assigned. |
| `UnknownUsbCOccupant` | `1` | Something is present in a USB-C slot, but type could not be identified. |
| `DpExpansionCard` | `2` | DisplayPort expansion card. |
| `HdmiExpansionCard` | `3` | HDMI expansion card. |
| `AudioExpansionCard` | `4` | Audio expansion card. |
| `Framework16KeyboardModule` | `5` | Framework 16 keyboard top-row/input module. |
| `Framework16LedMatrix` | `6` | Framework 16 LED matrix top-row module. |
| `Framework16TouchpadModule` | `7` | Framework 16 touchpad/input module. |
| `InternalKeyboard` | `8` | Built-in keyboard. |
| `InternalTouchpad` | `9` | Built-in touchpad. |
| `FingerprintReader` | `10` | Fingerprint reader. |
| `Touchscreen` | `11` | Internal touchscreen. |
| `Webcam` | `12` | Internal webcam. |
| `ExpansionBay` | `13` | Expansion bay present, generic classification only. |
| `ExpansionBayDualInterposer` | `14` | Expansion bay with dual interposer board. |
| `ExpansionBaySingleInterposer` | `15` | Expansion bay with single interposer board. |
| `ExpansionBayUmaFans` | `16` | Expansion bay with UMA/fan board. |
| `ExpansionBaySsdHolder` | `17` | Expansion bay SSD holder module. |
| `ExpansionBayPcieAccessory` | `18` | Expansion bay PCIe accessory module. |
| `ExpansionBayAmdGpu` | `19` | Expansion bay AMD GPU module. |
| `ExpansionBayNvidiaGpu` | `20` | Expansion bay NVIDIA GPU module. |
| `ExpansionBayFanOnly` | `21` | Expansion bay fan-only module. |
| `UsbAExpansionCard` | `22` | USB-A expansion card (USB hub). |
| `UsbCExpansionCard` | `23` | USB-C expansion card (passive passthrough). |
| `EthernetExpansionCard` | `24` | Ethernet 2.5G expansion card (Realtek RTL8156B). |
| `Ethernet10GExpansionCard` | `25` | Ethernet 10G expansion card (WisdPi, chip TBD). |
| `MicroSdExpansionCard` | `26` | MicroSD expansion card. |
| `SdExpansionCard` | `27` | Full-size SD expansion card. |
| `SsdExpansionCard` | `28` | NVMe storage expansion card (250GB 2nd Gen, 1TB 1st Gen). |

### `FrameworkModuleBus`

How a module was observed or classified.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `0` | Bus/provenance not known. |
| `Ec` | `1` | Derived from EC data. |
| `Usb` | `2` | Derived from USB enumeration. |
| `Hid` | `3` | Derived from HID enumeration. |
| `Composite` | `4` | Derived from multiple sources. |

### `FrameworkModuleSlotKind`

Logical slot/category a module belongs to.

| Name | Value | Meaning |
| --- | ---: | --- |
| `None` | `0` | No slot assigned. |
| `UsbCPort` | `1` | USB-C expansion-card slot. |
| `InputDeckTopRow` | `2` | Framework 16 top-row/input deck slot. |
| `InputDeckTouchpad` | `3` | Framework 16 touchpad/input deck position. |
| `ExpansionBay` | `4` | Expansion bay slot. |
| `InternalFixed` | `5` | Built-in fixed internal component. |
| `Detached` | `6` | Observed device not confidently mapped to a fixed slot. |
| `UsbCExpansionCardSlot` | `7` | Numbered Framework expansion card slot (slots 0–5 in the inventory). |

### `FrameworkExpansionCardType`

Discriminant for the type of card plugged into a USB-C expansion card slot.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `0` | Card type could not be determined. |
| `DisplayPort` | `1` | DisplayPort expansion card. |
| `Hdmi` | `2` | HDMI expansion card. |
| `Audio` | `3` | Audio expansion card. |
| `UsbA` | `4` | USB-A expansion card. |
| `UsbC` | `5` | USB-C expansion card (passive passthrough). |
| `Ethernet` | `6` | Ethernet 2.5G expansion card (Realtek RTL8156B). |
| `Ethernet10G` | `7` | Ethernet 10G expansion card (WisdPi, chip TBD). |
| `MicroSd` | `8` | MicroSD expansion card. |
| `Sd` | `9` | Full-size SD expansion card. |
| `Ssd` | `10` | NVMe storage expansion card. |

### `FrameworkPdTypeCState`

USB Type-C physical connection state.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Nothing` | `0` | No connection. |
| `Sink` | `1` | UFP/sink role. |
| `Source` | `2` | DFP/source role. |
| `Debug` | `3` | Debug accessory mode. |
| `Audio` | `4` | Audio accessory mode. |
| `PoweredAccessory` | `5` | Powered accessory mode. |
| `Unsupported` | `6` | Unsupported state. |
| `Invalid` | `7` | Unrecognized EC state value. |

### `FrameworkPdPowerRole`

USB PD power role.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Sink` | `0` | Consuming power. |
| `Source` | `1` | Providing power. |
| `Unknown` | `2` | Role could not be determined. |

### `FrameworkPdDataRole`

USB PD data role.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Ufp` | `0` | Upstream Facing Port (device). |
| `Dfp` | `1` | Downstream Facing Port (host). |
| `Disconnected` | `2` | Data disconnected. |
| `Unknown` | `3` | Role could not be determined. |

### `FrameworkPdCcPolarity`

USB-C CC pin orientation / debug-accessory mode.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `-1` | Polarity could not be determined. |
| `Cc1` | `0` | CC1 active (normal orientation). |
| `Cc2` | `1` | CC2 active (flipped orientation). |
| `Cc1Debug` | `2` | Debug accessory on CC1. |
| `Cc2Debug` | `3` | Debug accessory on CC2. |

### `FrameworkModuleConfidence`

Confidence level for a module classification.

| Name | Value | Meaning |
| --- | ---: | --- |
| `Unknown` | `0` | Confidence not set. |
| `DerivedWeak` | `1` | Heuristic/weak inference. |
| `DerivedStrong` | `2` | Strong heuristic inference. |
| `Direct` | `3` | Directly observed or explicitly reported by firmware/hardware. |

### `FrameworkModuleFlag` (Rust-side bitflags)

These bits explain `FrameworkModuleDescriptor.flags`.

| Name | Value | Meaning |
| --- | ---: | --- |
| `BuiltIn` | `1 << 0` | Module is built into the system. |
| `Active` | `1 << 1` | Module appears active/in-use. |
| `Connected` | `1 << 2` | Module/occupant appears electrically/logically connected. |
| `Fault` | `1 << 3` | Fault condition is present. |
| `Ambiguous` | `1 << 4` | Classification is ambiguous or tentative. |
| `HasPdContract` | `1 << 5` | USB-C path appears to have a PD contract. |
| `DisplayAltMode` | `1 << 6` | DisplayPort/alt-mode related state appears active. |
| `DoorClosed` | `1 << 7` | Expansion-bay door-closed signal is asserted. |
| `Enabled` | `1 << 8` | Module/bay is enabled. |

### `FrameworkEcFeatureFlagsResult`

Top-level EC feature-bit result.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the feature query succeeded. |
| `flags` | `ulong` | Bitwise OR of `FrameworkEcFeatureFlag` values. |

### `FrameworkEcKeyboardBacklightResult`

Keyboard backlight level result.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the query succeeded. |
| `brightness_percent` | `byte` | Current brightness as a percentage-like value. |
| `reserved` | `byte[3]` | Padding / future use. |

### `FrameworkEcFingerprintLedState`

Fingerprint LED state with both raw and interpreted forms.

| Field | Type | Meaning |
| --- | --- | --- |
| `raw_level` | `byte` | Raw firmware-reported LED level byte. |
| `reserved` | `byte[3]` | Padding / future use. |
| `level` | `FrameworkFingerprintLedLevel` | Interpreted LED mode/brightness. |

### `FrameworkEcFingerprintLedResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the query succeeded. |
| `state` | `FrameworkEcFingerprintLedState` | Fingerprint LED state payload. |

### `FrameworkEcExpansionBayStatus`

Expansion bay presence and classification.

| Field | Type | Meaning |
| --- | --- | --- |
| `present` | `byte` | Bool-like byte indicating whether the bay/module is present. |
| `enabled` | `byte` | Bool-like byte indicating whether the bay is enabled. |
| `fault` | `byte` | Bool-like byte indicating a fault condition. |
| `door_closed` | `byte` | Bool-like byte indicating the bay door/latch is closed. |
| `board` | `FrameworkExpansionBayBoard` | Board classification. |
| `vendor` | `FrameworkExpansionBayVendor` | Occupant/vendor family classification. |
| `config` | `FrameworkGpuPcieConfig` | Reported PCIe lane/gen configuration. |
| `reserved` | `byte[3]` | Padding / future use. |
| `serial_number` | `FrameworkByteBuffer` | UTF-8/raw serial bytes when available. Free with `framework_byte_buffer_free`. |

### `FrameworkEcExpansionBayStatusResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the query succeeded. |
| `bay` | `FrameworkEcExpansionBayStatus` | Expansion bay status payload. |

### `FrameworkGpuDescriptorHeader`

Header portion of a GPU descriptor blob.

| Field | Type | Meaning |
| --- | --- | --- |
| `magic` | `byte[4]` | Four-byte signature/magic identifying the descriptor format. |
| `length` | `uint` | Length of the header portion in bytes. |
| `desc_ver_major` | `ushort` | Descriptor format major version. |
| `desc_ver_minor` | `ushort` | Descriptor format minor version. |
| `hardware_version` | `ushort` | Hardware version value reported by the descriptor. |
| `hardware_revision` | `ushort` | Hardware revision value reported by the descriptor. |
| `serial` | `byte[20]` | 20-byte serial field; raw bytes, often zero-padded. |
| `descriptor_length` | `uint` | Length of the descriptor payload/body in bytes. |
| `descriptor_crc32` | `uint` | CRC32 of the descriptor payload/body. |
| `crc32` | `uint` | CRC32 of the header itself. |

### `FrameworkEcGpuDescriptorHeaderResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the header read succeeded. |
| `header` | `FrameworkGpuDescriptorHeader` | Parsed descriptor header. |

### `FrameworkEcGpuDescriptorReadResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the descriptor read succeeded. |
| `descriptor` | `FrameworkByteBuffer` | Raw full descriptor bytes (header + payload). Free with `framework_byte_buffer_free`. |

### `FrameworkEcGpuDescriptorValidationResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether validation completed successfully. |
| `is_match` | `byte` | Bool-like byte indicating whether the supplied descriptor bytes match the live descriptor. |
| `reserved` | `byte[3]` | Padding / future use. |

### `FrameworkModuleDescriptor`

Compact description of one detected or inferred module/slot occupant.

| Field | Type | Meaning |
| --- | --- | --- |
| `identity` | `FrameworkModuleIdentity` | Best-effort module classification. |
| `bus` | `FrameworkModuleBus` | Which source/bus produced the observation. |
| `slot_kind` | `FrameworkModuleSlotKind` | Logical slot/category for the module. |
| `confidence` | `FrameworkModuleConfidence` | Confidence level of the classification. |
| `present` | `byte` | Bool-like byte indicating whether the slot/module is considered populated/present. |
| `reserved_0` | `byte[3]` | Padding / future use. |
| `slot_index` | `int` | Zero-based index within the slot group when applicable. |
| `flags` | `uint` | Bitwise OR of `FrameworkModuleFlag` values. |
| `vendor_id` | `uint` | Observed vendor ID when available (for example USB/HID VID); otherwise usually zero. |
| `product_id` | `uint` | Observed product ID when available; otherwise usually zero. |
| `board_id` | `int` | Board/module-specific numeric identifier when available. |

### `FrameworkEcPdPortState`

Full USB PD port state from the EC (28 bytes). Embedded as a named field inside `FrameworkExpansionCardModuleDescriptor`.

| Field | Type | Meaning |
| --- | --- | --- |
| `c_state` | `FrameworkPdTypeCState` | Physical USB Type-C connection state. |
| `power_role` | `FrameworkPdPowerRole` | PD power role (sink/source). |
| `data_role` | `FrameworkPdDataRole` | PD data role (UFP/DFP). |
| `cc_polarity` | `FrameworkPdCcPolarity` | CC pin orientation. |
| `voltage_mv` | `ushort` | Negotiated voltage in millivolts. |
| `current_ma` | `ushort` | Negotiated current in milliamps. |
| `has_pd_contract` | `byte` | Bool-like byte; non-zero if a PD contract is active. |
| `vconn_active` | `byte` | Bool-like byte; non-zero if VCONN is active. |
| `epr_active` | `byte` | Bool-like byte; non-zero if EPR (Extended Power Range) is active. |
| `epr_support` | `byte` | Bool-like byte; non-zero if the port supports EPR. |
| `active_port` | `byte` | Bool-like byte; non-zero if this port is the active port. |
| `alt_mode_flags` | `byte` | Raw EC alt-mode status bits (bit 0: DP/TBT DFP_D, bit 1: UFP_D, bit 2: Power Low, bit 3: Enabled, bit 4: Multi-Function, bit 5: USB Config, bit 6: Exit Request, bit 7: HPD High). |
| `reserved` | `byte[2]` | Padding / future use. |

### `FrameworkExpansionCardModuleDescriptor`

Flat descriptor for one of the six numbered expansion card slots (64 bytes). All fields from `FrameworkModuleDescriptor` are inlined directly — no `@base` chain. `pd` is a coherent named sub-struct; all other fields are primitives/enums.

| Field | Type | Meaning |
| --- | --- | --- |
| `identity` | `FrameworkModuleIdentity` | Best-effort module classification. |
| `bus` | `FrameworkModuleBus` | Which source/bus produced the observation. |
| `slot_kind` | `FrameworkModuleSlotKind` | Always `UsbCExpansionCardSlot` for populated slots. |
| `confidence` | `FrameworkModuleConfidence` | Confidence in the slot-assignment (how sure we are which physical slot this is). |
| `present` | `byte` | Bool-like byte; non-zero if the slot appears populated. |
| `reserved_0` | `byte[3]` | Padding / future use. |
| `slot_index` | `int` | Zero-based slot index (0–5). |
| `flags` | `uint` | Bitwise OR of `FrameworkModuleFlag` values. |
| `vendor_id` | `uint` | Observed USB/HID vendor ID when available. |
| `product_id` | `uint` | Observed USB/HID product ID when available. |
| `board_id` | `int` | Board-specific identifier when available; otherwise `-1`. |
| `pd` | `FrameworkEcPdPortState` | Full PD port state for this slot. |
| `card_type` | `FrameworkExpansionCardType` | Identified card type. |
| `card_confidence` | `FrameworkModuleConfidence` | Confidence in the card-type identification (independent of slot-assignment confidence). |
| `reserved` | `byte` | Padding / future use. |

### `FrameworkModuleInventory`

Fixed-size inventory snapshot covering all known slot categories. Expansion card slots use `FrameworkExpansionCardModuleDescriptor` (flat, 64 bytes each with PD state and card type). All other slots use `FrameworkModuleDescriptor` directly (32 bytes each) — the `slot_kind` field distinguishes them semantically.

| Field | Type | Meaning |
| --- | --- | --- |
| `usb_c_slot_count` | `byte` | Number of meaningful USB-C slot entries in `usb_c_slot_0` ... `usb_c_slot_5`. |
| `input_top_row_count` | `byte` | Number of meaningful top-row/input deck entries in `input_top_row_0` ... `input_top_row_4`. |
| `detached_count` | `byte` | Number of meaningful detached entries in `detached_0` ... `detached_3`. |
| `reserved_0` | `byte` | Padding / future use. |
| `usb_c_slot_0` ... `usb_c_slot_5` | `FrameworkExpansionCardModuleDescriptor` | Per-expansion-card-slot descriptors; includes all module fields, PD state, and card type. |
| `input_top_row_0` ... `input_top_row_4` | `FrameworkModuleDescriptor` | Framework 16 top-row/input module positions (`slot_kind = InputDeckTopRow`). |
| `input_touchpad` | `FrameworkModuleDescriptor` | Framework 16 touchpad/input deck descriptor (`slot_kind = InputDeckTouchpad`). |
| `internal_keyboard` | `FrameworkModuleDescriptor` | Built-in keyboard (`slot_kind = InternalFixed`). |
| `internal_touchpad` | `FrameworkModuleDescriptor` | Built-in touchpad (`slot_kind = InternalFixed`). |
| `fingerprint_reader` | `FrameworkModuleDescriptor` | Fingerprint reader (`slot_kind = InternalFixed`). |
| `touchscreen` | `FrameworkModuleDescriptor` | Touchscreen (`slot_kind = InternalFixed`). |
| `webcam` | `FrameworkModuleDescriptor` | Webcam (`slot_kind = InternalFixed`). |
| `expansion_bay` | `FrameworkModuleDescriptor` | Expansion bay (`slot_kind = ExpansionBay`). |
| `detached_0` ... `detached_3` | `FrameworkModuleDescriptor` | Observed devices not confidently mapped to a fixed slot (`slot_kind = Detached`). |

### `FrameworkEcModuleInventoryResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `FrameworkStatus` | Whether the inventory build succeeded. |
| `inventory` | `FrameworkModuleInventory` | Full inventory snapshot payload. |

## Practical reading tips

- If a struct contains a `FrameworkByteBuffer`, copy out what you need and then free that buffer with `framework_byte_buffer_free`.
- If a result struct contains several `FrameworkByteBuffer` fields (for example `FrameworkBatterySnapshot` or `FrameworkEcFlashVersions`), each non-default buffer is owned independently.
- For any result with `status.code != Success`, treat the payload as informational only unless the specific API says otherwise.
- For inventory and expansion-bay booleans stored as bytes, use `value != 0` rather than comparing to `1` exactly.
- For fixed byte arrays such as GPU descriptor `magic` and `serial`, decode them explicitly; they are raw bytes, not automatic C# strings.
