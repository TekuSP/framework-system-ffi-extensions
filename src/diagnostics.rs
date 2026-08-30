//! EC diagnostic and system-state helpers.
//!
//! Backs the `hello`, protocol-info, sysinfo, panic-info, port-80 and switch
//! FFI entry points. Upstream exposes some of these only through printing
//! helpers, so the readback logic lives here and returns plain Rust values that
//! `lib.rs` maps onto the ABI structs.

use framework_lib::chromium_ec::commands::{EcRequestGetProtocolInfo, EcRequestSysinfo};
use framework_lib::chromium_ec::panic::PANIC_DATA_MAGIC;
use framework_lib::chromium_ec::{CrosEc, CrosEcDriver, EcError, EcRequestRaw};

/// EC memmap offset of the switch byte. Upstream keeps its `EC_MEMMAP_SWITCHES`
/// copy private to `power.rs`, so mirror it here like the other memmap offsets.
const EC_MEMMAP_SWITCHES: u16 = 0x30;

const EC_SWITCH_LID_OPEN: u8 = 0x01;
const EC_SWITCH_POWER_BUTTON_PRESSED: u8 = 0x02;
const EC_SWITCH_WRITE_PROTECT_DISABLED: u8 = 0x04;
const EC_SWITCH_DEDICATED_RECOVERY: u8 = 0x10;

/// The value `framework_ec_hello` sends when the caller does not supply one.
pub(crate) const HELLO_DEFAULT_IN_DATA: u32 = 0xa0b0_c0d0;
/// The EC echoes back `in_data + HELLO_OFFSET`.
pub(crate) const HELLO_OFFSET: u32 = 0x0102_0304;

pub(crate) struct Switches {
    pub raw: u8,
    pub lid_open: bool,
    pub power_button_pressed: bool,
    pub write_protect_disabled: bool,
    pub dedicated_recovery: bool,
}

/// Reads the EC switch byte. `None` when the memmap read fails.
pub(crate) fn get_switches(ec: &CrosEc) -> Option<Switches> {
    let raw = *ec.read_memory(EC_MEMMAP_SWITCHES, 1)?.first()?;
    Some(Switches {
        raw,
        lid_open: raw & EC_SWITCH_LID_OPEN != 0,
        power_button_pressed: raw & EC_SWITCH_POWER_BUTTON_PRESSED != 0,
        write_protect_disabled: raw & EC_SWITCH_WRITE_PROTECT_DISABLED != 0,
        dedicated_recovery: raw & EC_SWITCH_DEDICATED_RECOVERY != 0,
    })
}

pub(crate) struct ProtocolInfo {
    pub protocol_versions: u32,
    pub max_request_packet_size: u16,
    pub max_response_packet_size: u16,
    pub flags: u32,
}

pub(crate) fn get_protocol_info(ec: &CrosEc) -> Result<ProtocolInfo, EcError> {
    let res = EcRequestGetProtocolInfo {}.send_command(ec)?;
    Ok(ProtocolInfo {
        protocol_versions: res.protocol_versions,
        max_request_packet_size: res.max_request_packet_size,
        max_response_packet_size: res.max_response_packet_size,
        flags: res.flags,
    })
}

pub(crate) struct Sysinfo {
    pub reset_flags: u32,
    pub current_image: u32,
    pub flags: u32,
}

/// Upstream's `CrosEc::get_sysinfo` only prints, so send the command directly.
pub(crate) fn get_sysinfo(ec: &CrosEc) -> Result<Sysinfo, EcError> {
    let res = EcRequestSysinfo {}.send_command(ec)?;
    Ok(Sysinfo {
        reset_flags: res.reset_flags,
        current_image: res.current_image,
        flags: res.flags,
    })
}

pub(crate) struct PanicInfo {
    pub data: Vec<u8>,
    pub arch: u8,
    pub struct_version: u8,
    pub flags: u8,
    pub struct_size: u32,
    pub magic: u32,
    /// True when the trailer magic and struct size both agree with the payload.
    pub is_valid: bool,
}

/// `struct panic_data` header: arch, struct_version, flags, reserved.
const PANIC_HEADER_SIZE: usize = 4;
/// `struct panic_data` trailer: struct_size then magic, at the very end.
const PANIC_TRAILER_SIZE: usize = 8;

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// Reads stored EC panic data. An empty `data` means the EC has no saved panic.
pub(crate) fn get_panic_info(ec: &CrosEc) -> Result<PanicInfo, EcError> {
    let data = ec.get_panic_info()?;
    if data.len() < PANIC_HEADER_SIZE + PANIC_TRAILER_SIZE {
        return Ok(PanicInfo {
            arch: 0,
            struct_version: 0,
            flags: 0,
            struct_size: 0,
            magic: 0,
            is_valid: false,
            data,
        });
    }

    let struct_size = u32_at(&data, data.len() - 8);
    let magic = u32_at(&data, data.len() - 4);
    Ok(PanicInfo {
        arch: data[0],
        struct_version: data[1],
        flags: data[2],
        struct_size,
        magic,
        is_valid: magic == PANIC_DATA_MAGIC && struct_size as usize == data.len(),
        data,
    })
}

pub(crate) struct Port80History {
    pub writes: u32,
    pub history_size: u32,
    /// POST codes as little-endian `u16` pairs, in buffer order.
    pub codes: Vec<u8>,
}

pub(crate) fn port80_read(ec: &CrosEc) -> Result<Port80History, EcError> {
    let history = ec.port80_read()?;
    let mut codes = Vec::with_capacity(history.codes.len() * 2);
    for code in &history.codes {
        codes.extend_from_slice(&code.to_le_bytes());
    }
    Ok(Port80History {
        writes: history.writes,
        history_size: history.history_size,
        codes,
    })
}
