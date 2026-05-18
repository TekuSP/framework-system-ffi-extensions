# FFI Notes

This document tracks two things:

- the main feature gaps between `framework-system` and this standalone FFI facade
- the practical lessons learned while shaping the ABI for `csbindgen` and .NET

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
- keyboard backlight readback
- fingerprint LED readback
- expansion bay status snapshot
- GPU descriptor header readback
- raw GPU descriptor readback
- GPU descriptor validation against caller-provided full descriptor bytes
- unified module inventory snapshot with best-effort detection for USB-C cards,
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

- battery charge limit
- charge current limit
- charge rate limit
- other charger-oriented control surfaces

### Sensors and Switch State

- ambient light sensor values
- accelerometer data and lid angle
- privacy switch state
- intrusion switch state
- stylus battery reporting
- EC uptime and S0ix counters
- board ID reporting

### USB-C and PD Management

- PD port state details
- PD controller information
- PD reset/disable/enable operations
- Chromebook-style PD info surfaces

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
