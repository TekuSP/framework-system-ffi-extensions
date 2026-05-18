# Rust file overview

This document explains what each Rust file in the FFI crate does.

## Scope

This overview covers:

- `build.rs`
- `src/*.rs`
- `src/inventory/*.rs`

It does **not** document the vendored `framework-system/` submodule.

## Big picture

The crate is split into two layers:

- `src/lib.rs` keeps the public FFI ABI: exported `extern "C"` functions plus the C-friendly structs/enums/unions that `csbindgen` reads.
- helper modules hold the internal implementation so `lib.rs` stays focused on the ABI surface.

## Top-level Rust files

| File | What it does |
| --- | --- |
| `build.rs` | Generates `csharp/NativeMethods.g.cs` from `src/lib.rs` using `csbindgen`. This is why ABI-facing items still live in `lib.rs`. |
| `src/lib.rs` | Main FFI surface. Defines the public native ABI types and exports the C-callable entry points consumed by C# and other native callers. |
| `src/abi_impls.rs` | Conversion glue between upstream `framework_lib` Rust enums/types and the FFI enums exposed by this crate, such as EC driver, platform, platform family, and current EC image. |
| `src/byte_buffer.rs` | Owns `FrameworkByteBuffer` memory helpers: creating buffers from `Vec<u8>` and safely freeing them later across the FFI boundary. |
| `src/results.rs` | Small constructors for the various `*_Result` structs and a few default result payloads. Keeps repetitive result assembly out of `lib.rs`. |
| `src/runtime.rs` | Runtime/handle helpers: open the EC with the default or requested driver, validate incoming handle pointers, read feature flags, and parse optional fan indexes. |
| `src/status.rs` | Status plumbing. Builds `FrameworkStatus` values, maps EC errors to FFI status codes, stores device error strings behind integer tokens, and formats human-readable status descriptions. |
| `src/thermal.rs` | Thermal, fan, and power snapshot logic. Reads EC memmap thermal/fan data, converts it into FFI snapshot structs, and builds default thermal/power values when data is unavailable. |
| `src/inventory.rs` | Thin facade for the module inventory subsystem. It wires the inventory submodules together and re-exports the helpers that `lib.rs` calls. |

## Inventory submodule files

| File | What it does |
| --- | --- |
| `src/inventory/builder.rs` | Final inventory assembler. Decides slot counts, merges USB-C slot info, internal modules, and expansion bay data into one `FrameworkModuleInventory` value. |
| `src/inventory/conversions.rs` | Mapping helpers used by the inventory layer: module flags, fingerprint LED levels, expansion bay board/vendor/config values, and descriptor identity selection for cards/modules. |
| `src/inventory/defaults.rs` | Default constructors and result wrappers for inventory-related APIs, including feature flags, keyboard backlight, fingerprint LED, expansion bay status, and full module inventory results. |
| `src/inventory/detect.rs` | Local device discovery using `hidapi` and `rusb`. Detects expansion cards, audio cards, input modules, touchpads, touchscreens, webcams, and PD port observations. |
| `src/inventory/internals.rs` | Detects built-in and input-deck modules such as Framework 16 top-row modules, touchpad module, internal keyboard/touchpad, fingerprint reader, touchscreen, and webcam. |
| `src/inventory/query.rs` | EC-backed inventory queries. Reads expansion bay state and exposes a compact feature-flag summary derived from EC feature support. |
| `src/inventory/usb_slots.rs` | Populates USB-C slot descriptors and detached module slots. Matches PD observations with detected cards and handles Framework 16's 6-slot layout. |

## Quick routing guide

If you want to change something, this is usually the right file:

- **Add or change a public native API** → `src/lib.rs`
- **Change C# binding generation behavior** → `build.rs`
- **Adjust status codes or error descriptions** → `src/status.rs`
- **Adjust EC open/driver selection logic** → `src/runtime.rs`
- **Adjust FFI byte buffer ownership** → `src/byte_buffer.rs`
- **Adjust thermal/fan/power snapshots** → `src/thermal.rs`
- **Adjust module inventory composition** → `src/inventory/builder.rs`
- **Add a new inventory detector** → `src/inventory/detect.rs` or `src/inventory/internals.rs`
- **Adjust USB-C slot assignment logic** → `src/inventory/usb_slots.rs`
- **Adjust expansion bay or feature queries** → `src/inventory/query.rs`
- **Adjust mapping from upstream types to FFI enums** → `src/abi_impls.rs` or `src/inventory/conversions.rs`

## One important rule

Because `build.rs` points `csbindgen` at `src/lib.rs`, keep these in `src/lib.rs` unless the generation strategy changes:

- exported `extern "C"` functions
- public ABI structs/enums/unions

Everything else can usually live in helper modules.
