//! Firmware version readback for PD controllers, retimers and USB peripherals.
//!
//! Several upstream helpers in this area only print to stdout and return `()`
//! (`camera::check_camera_version`, `usbhub::check_usbhub_version`,
//! `inputmodule::check_inputmodule_version`, `audio_card::check_synaptics_fw_version`),
//! and some of them `unwrap()` on device errors. Panicking across the FFI boundary
//! aborts the host process, so this module re-implements the readbacks against
//! `rusb` directly, returning values and never panicking.

use std::time::Duration;

use framework_lib::ccgx::{
    self, AppVersion, Application, BaseVersion, ControllerFirmwares, ControllerVersion, PdVersions,
};
use framework_lib::chromium_ec::{CrosEc, EcError};
use framework_lib::parade_retimer;
use rusb::{Direction, GlobalContext, Recipient, RequestType};

/// Framework's USB vendor ID, shared by the audio card, camera and input modules.
const FRAMEWORK_VID: u16 = 0x32AC;
const AUDIO_CARD_PID: u16 = 0x0010;
const CAMERA_PIDS: [u16; 2] = [0x001C, 0x001D];

const REALTEK_VID: u16 = 0x0BDA;
const REALTEK_HUB_PIDS: [u16; 2] = [0x5432, 0x5424];
const GENESYS_VID: u16 = 0x05E3;
const GENESYS_HUB_PIDS: [u16; 1] = [0x0625];

const LEDMATRIX_PID: u16 = 0x0020;
const INPUT_MODULE_PIDS: [u16; 7] = [
    0x0012,
    0x0013,
    0x0014,
    0x0018,
    0x0019,
    0x0030,
    LEDMATRIX_PID,
];

/// A USB device's `bcdDevice` field, which is what Framework peripherals report as
/// their firmware version.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct UsbVersion {
    pub major: u8,
    pub minor: u8,
    pub sub_minor: u8,
}

/// One detected USB peripheral and its reported firmware version.
#[derive(Clone, Debug)]
pub(crate) struct PeripheralVersion {
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: UsbVersion,
    pub product_name: String,
}

fn usb_version(version: rusb::Version) -> UsbVersion {
    UsbVersion {
        major: version.major(),
        minor: version.minor(),
        sub_minor: version.sub_minor(),
    }
}

/// Enumerates USB devices matching `matches` and reports their `bcdDevice` version.
///
/// Opening a device to read its product string is best effort: on Linux the kernel
/// may hold the interface, and on Windows some devices refuse the open. A failed
/// open still yields the version, just with an empty product name.
fn collect_usb_versions(matches: impl Fn(u16, u16) -> bool) -> Vec<PeripheralVersion> {
    let Ok(devices) = rusb::devices() else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for device in devices.iter() {
        let Ok(descriptor) = device.device_descriptor() else {
            continue;
        };
        let vendor_id = descriptor.vendor_id();
        let product_id = descriptor.product_id();
        if !matches(vendor_id, product_id) {
            continue;
        }

        let product_name = device
            .open()
            .ok()
            .and_then(|handle| {
                descriptor
                    .product_string_index()
                    .and_then(|index| handle.read_string_descriptor_ascii(index).ok())
            })
            .unwrap_or_default();

        found.push(PeripheralVersion {
            vendor_id,
            product_id,
            version: usb_version(descriptor.device_version()),
            product_name,
        });
    }
    found
}

pub(crate) fn camera_versions() -> Vec<PeripheralVersion> {
    collect_usb_versions(|vid, pid| vid == FRAMEWORK_VID && CAMERA_PIDS.contains(&pid))
}

pub(crate) fn input_module_versions() -> Vec<PeripheralVersion> {
    collect_usb_versions(|vid, pid| vid == FRAMEWORK_VID && INPUT_MODULE_PIDS.contains(&pid))
}

pub(crate) fn usb_hub_versions() -> Vec<PeripheralVersion> {
    collect_usb_versions(|vid, pid| {
        (vid == REALTEK_VID && REALTEK_HUB_PIDS.contains(&pid))
            || (vid == GENESYS_VID && GENESYS_HUB_PIDS.contains(&pid))
    })
}

// ---------------------------------------------------------------------------
// Audio card (Synaptics CAPE over HID control transfers)
// ---------------------------------------------------------------------------

const CAPE_DATA_LEN: usize = 13;
const CAPE_MODULE_ID: u32 = 0xB32D_2300;
const CAPE_REPORT_ID: u16 = 0x0001;
const CAPE_GET_VERSION: u16 = 0x0103;
/// The card sets the top bit of the command id once a response is ready.
const CAPE_GET_VERSION_REPLY: u16 = 0x8103;
/// Upstream loops forever waiting for the reply; bound it instead so a wedged
/// card cannot hang the calling thread.
const CAPE_MAX_POLLS: usize = 32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct CapeMessage {
    len: i16,
    command_id: u16,
    module_id: u32,
    data: [u32; CAPE_DATA_LEN],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct HidCapeMessage {
    report_id: u16,
    msg: CapeMessage,
}

/// SAFETY: `HidCapeMessage` is `repr(C, packed)` and contains only integer fields,
/// so any bit pattern is a valid value and reading it as bytes is well defined.
unsafe fn as_bytes(value: &HidCapeMessage) -> &[u8] {
    std::slice::from_raw_parts(
        (value as *const HidCapeMessage) as *const u8,
        std::mem::size_of::<HidCapeMessage>(),
    )
}

/// SAFETY: see `as_bytes`.
unsafe fn as_bytes_mut(value: &mut HidCapeMessage) -> &mut [u8] {
    std::slice::from_raw_parts_mut(
        (value as *mut HidCapeMessage) as *mut u8,
        std::mem::size_of::<HidCapeMessage>(),
    )
}

fn find_hid_interface(handle: &rusb::DeviceHandle<GlobalContext>) -> Option<u8> {
    let config = handle.device().active_config_descriptor().ok()?;
    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            // 0x03 is the USB HID interface class.
            if descriptor.class_code() == 0x03 {
                return Some(interface.number());
            }
        }
    }
    None
}

/// Reads the audio expansion card's firmware version.
///
/// Returns `None` when no card is present or the CAPE exchange does not complete.
pub(crate) fn audio_card_version() -> Option<PeripheralVersion> {
    let devices = rusb::devices().ok()?;
    for device in devices.iter() {
        let Ok(descriptor) = device.device_descriptor() else {
            continue;
        };
        if descriptor.vendor_id() != FRAMEWORK_VID || descriptor.product_id() != AUDIO_CARD_PID {
            continue;
        }
        let Ok(handle) = device.open() else { continue };
        let Some(interface_number) = find_hid_interface(&handle) else {
            continue;
        };

        // On Linux a kernel driver claims this interface; on Windows the call is
        // unsupported and must not be treated as fatal.
        #[cfg(target_os = "linux")]
        let _ = handle.set_auto_detach_kernel_driver(true);

        if handle.claim_interface(interface_number).is_err() {
            continue;
        }

        let timeout = Duration::from_millis(100);
        let index = u16::from(interface_number);
        let request = HidCapeMessage {
            report_id: CAPE_REPORT_ID,
            msg: CapeMessage {
                len: (CAPE_DATA_LEN as i16).to_le(),
                command_id: CAPE_GET_VERSION.to_le(),
                module_id: CAPE_MODULE_ID,
                data: [0; CAPE_DATA_LEN],
            },
        };
        let mut response = request;

        let out_type = rusb::request_type(Direction::Out, RequestType::Class, Recipient::Interface);
        let in_type = rusb::request_type(Direction::In, RequestType::Class, Recipient::Interface);

        let mut answered = false;
        for _ in 0..CAPE_MAX_POLLS {
            // SetReport: report type 0x02 (output) in the high byte, report id in the low.
            let wrote = handle.write_control(
                out_type,
                0x09,
                u16::from_le_bytes([1, 0x02]),
                index,
                unsafe { as_bytes(&request) },
                timeout,
            );
            if wrote.is_err() {
                break;
            }
            // GetReport: report type 0x01 (input).
            let read = handle.read_control(
                in_type,
                0x01,
                u16::from_le_bytes([1, 0x01]),
                index,
                unsafe { as_bytes_mut(&mut response) },
                timeout,
            );
            if read.is_err() {
                break;
            }
            if { response.msg.command_id } == CAPE_GET_VERSION_REPLY {
                answered = true;
                break;
            }
        }

        let _ = handle.release_interface(interface_number);
        if !answered {
            continue;
        }

        // The version occupies the first four data words, one octet each.
        let data = { response.msg.data };
        let product_name = descriptor
            .product_string_index()
            .and_then(|i| handle.read_string_descriptor_ascii(i).ok())
            .unwrap_or_default();

        return Some(PeripheralVersion {
            vendor_id: FRAMEWORK_VID,
            product_id: AUDIO_CARD_PID,
            version: UsbVersion {
                major: data[0] as u8,
                minor: data[1] as u8,
                sub_minor: data[2] as u8,
            },
            product_name,
        });
    }
    None
}

// ---------------------------------------------------------------------------
// PD controllers and retimers
// ---------------------------------------------------------------------------

/// Upstream's `ControllerFirmwares` is neither `Clone` nor `Copy`, so this holds
/// the probed controllers by value and is moved rather than copied.
#[derive(Debug, Default)]
pub(crate) struct PdControllerSet {
    pub controllers: [Option<ControllerFirmwares>; 3],
}

/// Reads PD controller firmware versions.
///
/// Upstream returns a three-way enum (`Single` / `RightLeft` / `Many`); flatten it
/// into fixed slots so the ABI stays a plain record. Slot order follows upstream's
/// probe order: Right01, Left23, Back.
pub(crate) fn pd_controller_versions(ec: &CrosEc) -> Result<PdControllerSet, EcError> {
    let versions = ccgx::get_pd_controller_versions(ec)?;
    let mut set = PdControllerSet::default();
    match versions {
        PdVersions::Single(one) => set.controllers[0] = Some(one),
        PdVersions::RightLeft((right, left)) => {
            set.controllers[0] = Some(right);
            set.controllers[1] = Some(left);
        }
        PdVersions::Many(many) => {
            for (slot, firmware) in set.controllers.iter_mut().zip(many) {
                *slot = Some(firmware);
            }
        }
    }
    Ok(set)
}

pub(crate) fn retimer_version(ec: &CrosEc) -> Result<Option<Vec<u8>>, EcError> {
    parade_retimer::get_version(ec)
}

pub(crate) fn base_version_parts(version: &BaseVersion) -> (u8, u8, u8, u16) {
    (
        version.major,
        version.minor,
        version.patch,
        version.build_number,
    )
}

pub(crate) fn app_version_parts(version: &AppVersion) -> (i32, u8, u8, u8) {
    let application = match version.application {
        Application::Notebook => 0,
        Application::Monitor => 1,
        Application::AA => 2,
        Application::Invalid => 3,
    };
    (application, version.major, version.minor, version.circuit)
}

pub(crate) fn controller_version_parts(version: &ControllerVersion) -> (&BaseVersion, &AppVersion) {
    (&version.base, &version.app)
}

// ---------------------------------------------------------------------------
// NVMe
// ---------------------------------------------------------------------------

pub(crate) struct NvmeVersion {
    pub model_number: String,
    pub firmware_version: String,
}

/// Reads an NVMe drive's model and firmware version.
///
/// Upstream gates `framework_lib::nvme` behind `#[cfg(target_os = "linux")]`
/// because it issues an NVMe admin passthrough ioctl, so this returns `None` on
/// every other platform and the caller reports `NotSupported`.
#[cfg(target_os = "linux")]
pub(crate) fn nvme_version(device_path: &str) -> Option<NvmeVersion> {
    framework_lib::nvme::get_nvme_firmware_version(device_path)
        .ok()
        .map(|info| NvmeVersion {
            model_number: info.model_number,
            firmware_version: info.firmware_version,
        })
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn nvme_version(_device_path: &str) -> Option<NvmeVersion> {
    None
}

/// Whether NVMe readback is compiled in on this platform.
pub(crate) const NVME_SUPPORTED: bool = cfg!(target_os = "linux");
