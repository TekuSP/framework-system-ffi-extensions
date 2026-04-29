# framework_lib_ffi
[![CI](https://github.com/TekuSP/framework-system-ffi-extensions/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/TekuSP/framework-system-ffi-extensions/actions/workflows/ci.yml)

Standalone native FFI facade for `framework-system`, intended for .NET interop via
`csbindgen`.

This repository keeps the FFI crate separate from the upstream `framework-system`
repository and consumes upstream as a git submodule.

## Layout

- `framework-system/`: upstream Framework System repository as a git submodule
- `src/`: Rust FFI surface exported from this standalone crate
- `csharp/NativeMethods.g.cs`: generated low-level C# bindings

## Clone

Clone with submodules:

```sh
git clone --recurse-submodules <your-repo-url>
```

If you already cloned without submodules:

```sh
git submodule update --init --recursive
```

## Build

```sh
cargo build --release
```

Building regenerates `csharp/NativeMethods.g.cs` using `csbindgen`.

See `FFI_NOTES.md` for the current feature gaps and the main ABI/packaging learnings
from this work.

## Notes

- This crate depends on `framework-system/framework_lib` via a local path dependency.
- Update the `framework-system` submodule when you want to track upstream changes.
- `FrameworkByteBuffer` values returned by the native API must be released with
  `framework_byte_buffer_free` after the managed side has copied their contents.
