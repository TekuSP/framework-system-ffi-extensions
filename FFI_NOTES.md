# FFI Notes

This document tracks two things:

- the main feature gaps between `framework-system` and this standalone FFI facade
- the practical lessons learned while shaping the ABI for `csbindgen` and .NET

## Detection Reference

Complete list of every device the module inventory can detect, the exact VID:PID matched,
the detection method, and the confidence assigned. All VID:PIDs are matched from first
principles in `src/inventory/detect.rs` unless noted as upstream (sourced from `framework_lib`).

### Expansion Card Slots

Slot assignment uses a correlation pass: if exactly N of a card type are detected via USB/HID
and exactly N expansion card slots are unassigned and connected (no other match), the cards
are assigned to those slots with the confidence shown. Otherwise all detected cards go to
`detached`. `Direct` passes run before `DerivedWeak` passes so a higher-confidence match
locks a slot before a weaker pass can claim it.

| Card | VID:PID(s) | Chip / Origin | Detection bus | Confidence | Notes |
| --- | --- | --- | --- | --- | --- |
| DisplayPort (1st Gen) | `0x32AC:0x0003` | Cypress CCG3 | HID (upstream `ccgx::hid`) | **Direct** | Framework VID; DP firmware string |
| HDMI (1st Gen) | `0x32AC:0x0002` | Cypress CCG3 | HID (upstream `ccgx::hid`) | **Direct** | Framework VID; HDMI firmware string |
| DisplayPort (2nd Gen) | — | Passive DP alt-mode passthrough | EC `alt_mode_flags` bit 0/1 | **DerivedWeak** | No USB device; cannot distinguish from HDMI 3rd Gen |
| HDMI (3rd Gen) | — | Parade PS186 | EC `alt_mode_flags` bit 0/1 | **DerivedWeak** | No USB device; cannot distinguish from DP 2nd Gen |
| Audio | `0x32AC:0x0010` | Framework firmware | USB (upstream `audio_card`) | **Direct** | Framework VID + unique PID; unambiguous |
| Storage (1TB 1st Gen, 250GB 2nd Gen) | `0x32AC:0x0005` | Framework firmware | USB (`detect_ssd_cards_local`) | **Direct** | Framework VID + unique PID; covers both generations |
| USB-A (1st/2nd Gen) | `0x0BDA:0x5432` | Realtek RTL8153 | USB (`detect_usb_a_cards_local`) | **DerivedWeak** | Generic hub chip; appears in non-card contexts |
| USB-A (1st/2nd Gen) | `0x0BDA:0x5424` | Realtek RTL8153 | USB (`detect_usb_a_cards_local`) | **DerivedWeak** | Generic hub chip; appears in non-card contexts |
| USB-A (1st/2nd Gen) | `0x05E3:0x0625` | Genesys Logic GL3590 | USB (`detect_usb_a_cards_local`) | **DerivedWeak** | Generic hub chip; appears in non-card contexts |
| Ethernet 2.5G | `0x0BDA:0x8156` | Realtek RTL8156B | USB (`detect_ethernet_cards_local`) | **DerivedWeak** | NIC chip; appears in USB-C docks |
| SD (full-size) | `0x05E3:0x0749` | Genesys Logic GL3230 | USB (`detect_sd_cards_local`) | **DerivedWeak** | Card reader chip; unverified on real hardware |
| MicroSD | `0x05E3:0x0751` | Genesys Logic | USB (`detect_microsd_cards_local`) | **DerivedWeak** | Card reader chip; unverified on real hardware |
| USB-C (all colors) | — | Passive passthrough | None | **Unknown** | No USB device; no distinguishing EC signal |
| Ethernet 10G (WisdPi) | — | Chip TBD | None | **Unknown** | Not yet shipping; no known VID:PID |

### Internal Components

Internal components are assigned directly to named inventory fields (not via slot correlation).
No slot-assignment uncertainty exists — the inventory field name is the type tag.

| Component | VID:PID | Chip / Origin | Detection bus | Confidence | Notes |
| --- | --- | --- | --- | --- | --- |
| Internal keyboard (FW13/12/Desktop) | — | — | EC `EcFeatureCode::Keyboard` | **DerivedStrong** | No USB ID; feature flag only |
| Internal touchpad (FW13/12/Desktop) | `0x093A:0x0274` | PixArt | HID (usage page `0xFF00`) | **Direct** | |
| Internal touchpad (FW13/12/Desktop) | `0x093A:0x0239` | PixArt | HID (usage page `0xFF00`) | **Direct** | |
| Internal touchpad (FW13/12/Desktop) | `0x093A:0x0360` | PixArt | HID (usage page `0xFF00`) | **Direct** | |
| Internal touchpad (FW13/12/Desktop) | `0x093A:0x0343` | PixArt | HID (usage page `0xFF00`) | **Direct** | |
| FW16 touchpad module | same as above | PixArt | HID + board ID from EC | **Direct** | Board ID read via EC `BoardIdType::Touchpad` |
| FW16 top-row keyboard modules | `0x32AC:0x0012` … `0x0019`, `0x0030` | Framework firmware | USB + physical port numbers | **Direct** | Port numbers `[4,2]`–`[3,3]` map to top-row slots 0–4 |
| FW16 LED matrix module | `0x32AC:0x0020` | Framework firmware | USB + physical port numbers | **Direct** | Same port-number mapping as keyboard modules |
| FW16 input deck (EC path) | — | — | EC `get_input_deck_status()` | **Direct** | Gives module type + touchpad board ID per slot |
| Fingerprint reader | — | — | EC `EcFeatureCode::Fingerprint` | **DerivedStrong** | Feature flag only |
| Fingerprint reader (with LED) | — | — | EC `get_fp_led_level()` | **Direct** | LED readback confirms reader is present and active |
| Touchscreen (ILI Technology) | `0x222A:0x5539` | ILI Technology | HID (usage page `0xFF00`, upstream `touchscreen`) | **Direct** | |
| Touchscreen (Himax HX) | `0x3558:0x14FD` | Himax | HID (upstream `touchscreen`) | **Direct** | |
| Webcam (FW13/16 2nd Gen) | `0x32AC:0x001C` | Framework firmware | USB (upstream `camera`) | **Direct** | Framework VID + unique PID |
| Webcam (FW12) | `0x32AC:0x001D` | Framework firmware | USB (upstream `camera`) | **Direct** | Framework VID + unique PID |

### Expansion Bay

The expansion bay is detected via a single EC command rather than USB/HID enumeration.

| Slot | VID:PID | Detection method | Confidence | Notes |
| --- | --- | --- | --- | --- |
| Expansion bay | — | EC `get_expansion_bay_status()` | **Direct** | Returns board type, vendor, PCIe config, fault/door state |

---

## Current Scope

The current FFI covers the main building blocks needed for a .NET thermal and fan
control layer:

- EC open/close and driver selection
- platform, platform family, and product name
- EC build info and flash version strings
- power snapshot
- thermal snapshot
- fan capability reporting
- fan RPM control
- fan duty control
- restore automatic fan control
- compact EC feature flags for common presence/control checks
- keyboard backlight readback and write
- fingerprint LED readback and write (`Unknown`/`Custom` rejected; `Custom` is get-only per EC)
- privacy switches read (microphone enabled, camera enabled)
- battery charge limits read and write (min%, max%)
- charge current limit set (mA, optional battery SoC threshold)
- chassis intrusion read (currently open, ever opened, open count, VTR open count, coin cell removed)
- EC uptime (ms since boot, AP reset count, EC reset flags)
- S0ix counter read and reset
- tablet mode override write (Framework 12/13; returns InvalidCommand on other platforms)
- Framework 16 input deck mode write
- expansion bay status snapshot
- GPU descriptor header readback
- raw GPU descriptor readback
- GPU descriptor validation against caller-provided full descriptor bytes
- unified module inventory snapshot with best-effort detection for USB-C expansion cards
  (DP/HDMI via HID, Audio/SSD via USB VID:PID, USB-A/Ethernet/SD/MicroSD via USB hub
  chip PIDs), PD port state per slot (voltage, current, power role, data role, alt-mode),
  Framework 16 input modules, touchpad, fingerprint reader, touchscreen, webcam,
  and expansion bay presence
- structured status and device error reporting
- EC thermal thresholds read and write per sensor (warn / high / halt, their release
  points, and the `fan_off` / `fan_max` temperatures)
- EC-reported temperature sensor names and sensor type tags, with the name resolved onto
  the stable `FrameworkSensorName` enum
- AP throttle status (soft and hard)
- battery cutoff (ship mode) status
- EC switch positions (lid open, power button, write protect, dedicated recovery)
- EC `hello` liveness echo and a `check_hello` convenience wrapper
- EC host command protocol info (supported versions, max packet sizes)
- EC sysinfo (current image, reset flags, sysinfo flags)
- saved EC panic data as a raw blob with decoded header/trailer fields
- port 80 POST code history
- full Smart Battery (SBS) data set, including chemistry, manufacture date, cycle count,
  per-cell voltages, safety/PF status words and the lifetime blocks when unsealed
- Smart Battery SHA-1 HMAC authentication challenge
- PD controller firmware versions (bootloader, backup and main images per controller)
- PD charger info per port (role, charging type, voltage/current limits, max power)
- charging and AC-present state
- retimer firmware version
- EC-reported fan count
- standalone (batteryless Desktop) mode state
- charge rate limit write
- EC hibernate delay read and write
- fingerprint LED percentage write
- raw ADC channel read
- host command version probing
- stylus battery level
- touchscreen enable/disable
- touchpad haptic intensity and click force (write-only)
- per-key RGB keyboard colors
- key remapping and Caps-to-Ctrl
- PS/2 emulation toggle
- GPIO read, write, and enumeration by index
- expansion-bay GPU serial read (the write path is deliberately not exposed)
- firmware versions for cameras, Framework 16 input modules, USB hubs and the audio card
- NVMe model and firmware version (Linux only)

The current FFI still does **not** expose a max-fan-RPM reader. The `fan_off` / `fan_max`
values now readable through `framework_ec_get_thermal_thresholds` are the *temperature*
setpoints at which the EC starts and maxes active cooling — they are not RPM limits. The
repo reads live fan RPM and can set a target RPM/duty, but the "limited by EC fan table max
RPM" behavior remains firmware-enforced rather than a separate readable FFI value.

## Missing Features

Compared with the full `framework-system` repo and CLI, the major missing areas are:

### Charger and Battery Controls

Nothing outstanding. Charge limits, charge current limit, charge rate limit, cutoff
status, charging/AC state and the full Smart Battery data set are all exposed.

### Sensors and Switch State

Nothing outstanding. Ambient light, accelerometer/lid angle, EC uptime, S0ix counters,
board ID, EC switch positions, AP throttle status, raw ADC channels and stylus battery
are all exposed.

### USB-C and PD Management

- PD reset/disable/enable operations
- Chromebook-style PD info surfaces
- PD bus locking (`lock_pd_bus`) — deliberately unexposed, see below

PD controller firmware versions, per-port charger info and retimer versions are exposed now.
- USB-C expansion card VID/PID confirmation: SD (`0x05E3:0x0749`) and MicroSD (`0x05E3:0x0751`) PIDs are Genesys Logic reader candidates; confidence is DerivedWeak pending hardware testing against actual Framework cards
- USB-C passive cards (USB-C expansion card, DP 2nd Gen passthrough, HDMI 3rd Gen Parade PS186) have no USB presence; they remain Unknown/DerivedWeak with no slot disambiguation path currently

### Device and Platform Controls

- NVIDIA-related status on supported systems (upstream gates this behind its optional
  `nvidia` feature and `nvml-wrapper`, which this crate does not enable)

Fingerprint LED level and percentage, tablet mode override, input deck mode, touchscreen
enable/disable, touchpad haptics and click force, per-key RGB, key remapping, PS/2
emulation, GPIO access and hibernate delay are all exposed now.

### Firmware and Binary Tooling

- ESRT access
- firmware version surfaces beyond the currently exposed subset
- GPU descriptor writing / flashing
- EC and PD binary parsing
- capsule parsing
- EC reboot and image-jump controls
- EC flash dumping and flashing
- structured decoding of the panic blob (`framework_ec_get_panic_info` returns the raw
  `struct panic_data` plus header/trailer fields; upstream's `chromium_ec::panic` keeps its
  per-architecture decode structs private, so only `print_panic_info` exists there)

### Expansion Card and Peripheral Support

- DisplayPort and HDMI expansion card update flows
- audio card detail beyond the firmware version

Camera, input module, USB hub and audio card firmware versions, retimer version and
NVMe model/firmware are exposed now. Note the NVMe path is **Linux only**: upstream gates
`framework_lib::nvme` behind `#[cfg(target_os = "linux")]` because it issues an NVMe admin
passthrough ioctl, so other platforms get `FrameworkStatusCode::NotSupported`.

### Raw and Advanced Escape Hatches

- generic host command bridge
- more direct feature-query surfaces
- EC console read (`console_read_one` returns a `String` and would be cheap to add)
- full memmap dump (`dump_mem_region`)
- self-test style operations

GPIO read, write and enumeration are exposed now, as is host command version probing.

## Highest-Value Next Features

For a .NET application focused on fan curves, system telemetry, and machine status,
the highest-value remaining additions are likely:

1. a generic feature-query API (the current surface exposes a compact curated flag set)
2. a raw host-command escape hatch if fast parity matters more than a curated ABI
3. a max-fan-RPM reader, so managed fan curves can normalise duty against the real ceiling
4. ESRT, for firmware inventory and update status
5. structured panic decoding, if EC crash triage moves into the managed app
6. EC console read and memmap dump, both cheap, for diagnostics

## Deliberately Not Exposed

These upstream capabilities are reachable but intentionally left out of the ABI. Exposing
them through a .NET interop layer puts destructive firmware operations one P/Invoke away
from any managed caller, with no confirmation path and no way to recover a bricked EC.

- **EC flash**: `reflash`, `read_ec_flash`, `get_entire_ec_flash`, `protect_ec_flash`,
  `test_ec_flash_read`, `flash_notify`
- **EC reboot and image jump**: `reboot`, `reboot_ec`, `jump_ro`, `jump_rw`,
  `disable_jump`, `cancel_jump`
- **GPU descriptor and serial writing**: `set_gpu_descriptor`, `write_ec_gpu_chunk`,
  `set_gpu_serial`. The descriptor *read* and *validate* paths are exposed, as is
  `framework_ec_get_gpu_serial` — only the write paths are withheld. Programming a serial
  changes persistent expansion-bay identity, and upstream's `set_gpu_serial` copies into a
  fixed slice without checking the length.
- **Retimer firmware update mode**: `retimer_enable_fwupd`, `retimer_enable_compliance`
- **PD bus locking**: `lock_pd_bus`

Also left out, for a different reason: the `ec_binary`, `capsule` and `ccgx::binary`
parsers operate on file bytes rather than hardware, so they are pure functions the managed
side can implement directly without crossing the FFI boundary. The `smart_battery` file and
console helpers are skipped on the same grounds.

## Submodule Update History

### 2026-08-30: framework-system 39f0f89 → a338c6a (v0.6.5-19)

38 upstream commits. **No breaking changes** — every change to the `framework_lib`
surface this crate consumes was additive, and the crate compiled against the new
submodule with no source edits required.

**Upstream additions that mattered:**

- `CrosEc::get_temp_sensor_name(id)` — EC firmware now reports sensor names, and upstream
  **deleted** the hardcoded per-platform sensor table that our `thermal::sensor_name()`
  mirrors (`7c3e552 --thermal: Remove hardcoded names`)
- `CrosEc::get_thermal_threshold` / `set_thermal_threshold` and `EcThermalConfig`
  (warn/high/halt plus `temp_fan_off`/`temp_fan_max`)
- `CrosEc::get_ap_throttle_status()` — soft/hard AP throttle
- `CrosEc::hello()` / `check_hello()`, `get_protocol_info()`, `get_panic_info()`,
  `port80_read()`, and the `chromium_ec::panic` module
- `EcRequestSysinfo` plus `SysinfoFlag` (upstream's `get_sysinfo()` only prints)
- `power::get_cutoff_status()` — battery ship-mode state
- `power::print_switches()` and a now-public `EC_MEMMAP_SWITCHES` read path
- the `smart_battery` module: `SmartBattery::collect_data(ec, unseal_key) -> BatteryData`
  and `SmartBattery::authenticate_battery(ec, &[u8; 16]) -> bool`
- MEC/NPC EC flash layout corrections and `ccgx` byte-literal cleanups (no FFI impact)

**FFI impact: 15 new exported functions.** See the Current Scope list below. The existing
ABI was **not** changed — no struct layouts moved, no enum values were renumbered, and no
functions were removed or resigned. Everything new is additive, so existing managed callers
keep working after regenerating bindings.

**Sensor naming decision.** `thermal::sensor_name()` keeps the static per-platform table
even though upstream dropped its copy: it costs no host commands, so the polled thermal
snapshot stays cheap. The authoritative firmware name is exposed separately through
`framework_ec_get_temp_sensor_name`, which also resolves the name onto the stable
`FrameworkSensorName` enum via `thermal::map_sensor_name`. Managed callers should read
names once, cache them, and keep polling the snapshot for values.

That entry point sends `EcRequestTempSensorGetInfo` directly rather than calling upstream's
`CrosEc::get_temp_sensor_name`, because the helper discards the `sensor_type` byte the EC
returns in the same response. One command, both halves.

**Not mirrored from upstream.** `smart_battery`'s file and console helpers
(`BatteryData::write_to_file` / `read_from_file`, `dump_data`, `dump_to_file`,
`display_battery_data`, `analyze_health`, `interactive_authenticate`) stay unexposed by
design: they are persistence and stdout concerns that belong in the managed layer, and the
underlying values they format are all reachable through `framework_ec_get_smart_battery_data`.
Upstream's other new printing helpers (`print_thermal_thresholds`, `print_switches`,
`print_panic_info`, `get_sysinfo`) are likewise superseded by the structured readbacks.

**Toolchain note.** Upstream's new `smart_battery.rs` trips the `io_other_error` lint on
current stable clippy. `framework_lib` is a path dependency, so clippy treats it as a local
package and `cargo clippy -- -D warnings` fails on upstream's lint debt. CI now runs
`cargo clippy --no-deps --all-targets -- -D warnings` to keep the gate on this crate.

Build, fmt, clippy, and the 13 unit tests all pass after the update.

### 2026-06-23: framework-system 993cb6b → 39f0f89 (v0.6.4)

**Commits included:**

- `73f38d8` --test: Fix issues on desktop (selftest PD handling for Desktop platform)
- `5e6f4ef` --thermal: decode temp 4 (AMD Desktop adds "Virtual" sensor display)
- `1cf031f` --pdports: Gracefully handle non-existent ports
- `90e7d56` --thermal: Decode fan names (APU Fan, Left Fan, Right Fan, Front Fan, Third Fan)
- `7bb3870` bump version to 0.6.4
- `ab7fa58`, `58cd5ed`, `ce3abb7`, `39f0f89` contrib/README/doc changes only

**FFI impact: none.** All code changes were in display/print helper functions (`print_thermal`, `get_and_print_cypd_pd_info`, `selftest`) which the FFI crate does not call. The `framework_lib` public API surface is unchanged. No new FFI bindings were required. Build, fmt, and clippy all pass cleanly after the update.

The `Laptop 13 Pro (Intel Core Ultra Series 3)` SMBIOS string mapping (→ `Platform::IntelCoreUltra3`) was already present in the prior commit and already exposed in our FFI as `FrameworkPlatform::IntelCoreUltra3 = 12`.

## Learnings

### ABI Shape

- The generated C# is the right review artifact. If the generated shape feels wrong,
  fix the Rust ABI instead of layering handwritten C# wrappers over it.
- By-value result structs work better than out parameters for this surface.
- A shared `FrameworkStatus` field on result records gives consistent error handling
  without turning every API into a special case.
- Nested structs and enums generate much better interop shapes than flat flags and
  unrelated primitive fields.
- Bitmask-style capability fields currently generate as primitive `ulong` / `uint`
  values in C# rather than named flag enums, so the managed side should keep named
  constants/helpers for `FrameworkEcFeatureFlagsResult.flags` and
  `FrameworkModuleDescriptor.flags`.
- Fixed-size byte arrays generate as C# `fixed byte[...]` buffers, which works well
  for truly binary fixed-layout metadata such as GPU descriptor `magic` and `serial`
  fields but is still a poor fit for general-purpose strings.
- **Flat independent structs over C-inheritance chains.** The C first-field casting
  idiom (`base` as first field) produces deep `@base.@base.@base.field` chains in C#
  that `@` escape a reserved keyword and break every access path. .NET best practice
  for FFI is flat, independent structs: inline shared fields directly, use a coherent
  named sub-struct only where the sub-struct is a meaningful semantic group (e.g.
  `FrameworkEcPdPortState pd`). Slot types with no extra data beyond `FrameworkModuleDescriptor`
  use `FrameworkModuleDescriptor` directly; the `slot_kind` field carries the semantic
  type tag. The managed layer provides typed records/classes via `typeof`/`is`, not
  the FFI structs.

### Strings and Ownership

- Dynamic text should be exposed as `FrameworkByteBuffer` instead of fixed byte arrays
  when the managed side needs to treat the data as strings.
- Every returned `FrameworkByteBuffer` must be freed with `framework_byte_buffer_free`
  after its contents have been copied.
- This applies to nested buffers too, such as battery text fields, flash version
  strings, and raw GPU descriptor blobs.
- Once a record carries more than a handful of buffers, a dedicated aggregate free is
  better than making the managed side walk every field. `FrameworkSmartBatteryData` owns
  ten buffers and is released with `framework_smart_battery_data_free`, which nulls each
  field as it frees so a double call is harmless. Records with one or two buffers stay on
  the plain `framework_byte_buffer_free` convention.
- Per-call cost belongs in the ABI shape, not just the docs. EC-reported temperature sensor
  names cost one host command each, so they are a separate `framework_ec_get_temp_sensor_name`
  call rather than eight extra buffers on every polled thermal snapshot.

### Status and Error Reporting

- Rich status payloads are worth keeping in Rust so `csbindgen` can generate a useful,
  low-level interop layer without handwritten glue.
- Device error messages are better exposed indirectly through a token plus retrieval API
  than discarded into a generic error code.

### Panic Safety Across the Boundary

- **A panic in Rust aborts the managed host.** Several upstream helpers `unwrap()` on
  device errors or index fixed slices without checking, which is fine for a CLI that
  crashes with a message and unacceptable for a library loaded into a long-running app.
  Wrap or re-implement rather than calling them directly.
- Cases found so far: `CrosEc::get_gpu_serial` (`String::from_utf8(..).unwrap()`),
  `audio_card::check_synaptics_fw_version` (`unwrap()` on device open plus an
  `assert_eq!` on the HID reply), and the `camera` / `usbhub` / `inputmodule` version
  helpers (`rusb::devices().unwrap()`). `CrosEc::set_gpu_serial` has the same problem
  (`copy_from_slice` on a serial that must be exactly 18 bytes), but it is not exposed at
  all — see Deliberately Not Exposed.
- Unbounded waits are the same class of problem. Upstream's audio card version loop spins
  until the card replies; `versions::audio_card_version` bounds it instead so a wedged
  card cannot hang the calling thread.
- When re-implementing, keep upstream's protocol semantics exactly and change only the
  error handling — the point is to return a `FrameworkStatus`, not to behave differently.

### Platform Availability

- Some upstream modules are `cfg`-gated and simply do not exist on every target that CI
  builds. `framework_lib::nvme` is `#[cfg(target_os = "linux")]`, so `framework_get_nvme_version`
  compiles to a stub elsewhere.
- `FrameworkStatusCode::NotSupported` exists to distinguish "this build cannot do that on
  this platform" from `DataUnavailable`, which means the capability is present but did not
  answer. Managed callers should treat the former as a permanent capability gap and the
  latter as a transient read failure.

### Upstream Compatibility

- When mirroring upstream enums such as platform identifiers, keep numeric stability in
  the FFI representation. Append new values instead of renumbering older ones.
- Clean upstream `framework_lib` does not currently expose the thermal helper types and
  functions used on the `dotnet_ffi` branch, so this standalone repo keeps its own
  thermal snapshot parsing in `src/lib.rs`.

### Standalone Repository Packaging

- A standalone nested repo copy of the crate needs an empty `[workspace]` table in its
  own `Cargo.toml` so Cargo does not try to inherit the outer workspace.
- Using the upstream repo as a git submodule is technically straightforward and keeps
  the FFI repository independent from upstream merge decisions.
- A separate repo means you own the update cadence when upstream `framework_lib`
  changes.

### Push vs Pull

- The current FFI is synchronous request/response only.
- An `IObservable`-style experience is still easy to build in C# by polling snapshots
  and emitting changes from managed code.
- True native push semantics are possible, but they would require callback registration,
  background workers, unsubscribe handles, and stricter lifetime/threading rules across
  the FFI boundary.
