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

The current FFI still does **not** expose an implemented EC fan-table or max-fan-RPM
reader. The repo currently reads live fan RPM and can set a target RPM/duty, but the
"limited by EC fan table max RPM" behavior remains firmware-enforced rather than a
separate readable FFI value.

## Missing Features

Compared with the full `framework-system` repo and CLI, the major missing areas are:

### Charger and Battery Controls

- charge rate limit
- other charger-oriented control surfaces

### Sensors and Switch State

- ambient light sensor values
- accelerometer data and lid angle
- stylus battery reporting
- EC uptime and S0ix counters
- board ID reporting

### USB-C and PD Management

- PD controller information
- PD reset/disable/enable operations
- Chromebook-style PD info surfaces
- USB-C expansion card VID/PID confirmation: SD (`0x05E3:0x0749`) and MicroSD (`0x05E3:0x0751`) PIDs are Genesys Logic reader candidates; confidence is DerivedWeak pending hardware testing against actual Framework cards
- USB-C passive cards (USB-C expansion card, DP 2nd Gen passthrough, HDMI 3rd Gen Parade PS186) have no USB presence; they remain Unknown/DerivedWeak with no slot disambiguation path currently

### Device and Platform Controls

- broader keyboard backlight control surface
- fingerprint LED write/control surface
- tablet mode override
- touchscreen enable/disable
- input deck mode control
- NVIDIA-related status on supported systems

### Firmware and Binary Tooling

- ESRT access
- firmware version surfaces beyond the currently exposed subset
- GPU descriptor writing / flashing
- EC and PD binary parsing
- capsule parsing
- EC reboot and image-jump controls
- EC flash dumping and flashing

### Expansion Card and Peripheral Support

- richer DisplayPort and HDMI expansion card details and update flows
- richer audio card info
- retimer and other peripheral-oriented surfaces

### Raw and Advanced Escape Hatches

- generic host command bridge
- GPIO access
- more direct feature-query surfaces
- self-test style operations

## Highest-Value Next Features

For a .NET application focused on fan curves, system telemetry, and machine status,
the highest-value additions are likely:

1. charge-limit and charger-related APIs
2. a generic feature-query API
3. privacy, intrusion, and other switch/sensor state APIs
4. keyboard backlight and similar user-facing device controls
5. a raw host-command escape hatch if fast parity matters more than a curated ABI

## Submodule Update History

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

### Status and Error Reporting

- Rich status payloads are worth keeping in Rust so `csbindgen` can generate a useful,
  low-level interop layer without handwritten glue.
- Device error messages are better exposed indirectly through a token plus retrieval API
  than discarded into a generic error code.

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
