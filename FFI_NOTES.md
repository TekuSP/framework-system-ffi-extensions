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
- structured status and device error reporting

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

- keyboard backlight control
- fingerprint LED control
- tablet mode override
- touchscreen enable/disable
- input deck status and mode
- expansion bay status
- NVIDIA-related status on supported systems

### Firmware and Binary Tooling

- ESRT access
- firmware version surfaces beyond the currently exposed subset
- EC and PD binary parsing
- capsule parsing
- EC reboot and image-jump controls
- EC flash dumping and flashing

### Expansion Card and Peripheral Support

- DisplayPort and HDMI expansion card info and update flows
- audio card info
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

### Strings and Ownership

- Dynamic text should be exposed as `FrameworkByteBuffer` instead of fixed byte arrays
  when the managed side needs to treat the data as strings.
- Every returned `FrameworkByteBuffer` must be freed with `framework_byte_buffer_free`
  after its contents have been copied.
- This applies to nested buffers too, such as battery text fields and flash version
  strings.

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
