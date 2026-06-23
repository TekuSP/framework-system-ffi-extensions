use framework_lib::audio_card;
use framework_lib::camera;
use framework_lib::ccgx::hid;
use framework_lib::inputmodule;
use framework_lib::touchpad;
use framework_lib::touchscreen;
use hidapi::HidApi;

use crate::*;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct HidModuleObservation {
    pub(super) vendor_id: u16,
    pub(super) product_id: u16,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct UsbModuleObservation {
    pub(super) vendor_id: u16,
    pub(super) product_id: u16,
    pub(super) slot_index: i32,
}

pub(super) fn detect_expansion_cards_local() -> Vec<HidModuleObservation> {
    let api = match HidApi::new() {
        Ok(api) => api,
        Err(_) => return Vec::new(),
    };

    hid::find_devices(&api, &hid::ALL_CARD_PIDS, None)
        .into_iter()
        .map(|device| HidModuleObservation {
            vendor_id: device.vendor_id(),
            product_id: device.product_id(),
        })
        .collect()
}

pub(super) fn detect_ssd_cards_local() -> Vec<UsbModuleObservation> {
    const FRAMEWORK_VID: u16 = 0x32AC;
    const SSD_EXPANSION_CARD_PID: u16 = 0x0005;

    let devices = match rusb::devices() {
        Ok(devices) => devices,
        Err(_) => return Vec::new(),
    };

    devices
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            if descriptor.vendor_id() == FRAMEWORK_VID
                && descriptor.product_id() == SSD_EXPANSION_CARD_PID
            {
                Some(UsbModuleObservation {
                    vendor_id: descriptor.vendor_id(),
                    product_id: descriptor.product_id(),
                    slot_index: -1,
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn detect_usb_a_cards_local() -> Vec<UsbModuleObservation> {
    const REALTEK_VID: u16 = 0x0BDA;
    const RTL5432_PID: u16 = 0x5432;
    const RTL5424_PID: u16 = 0x5424;
    const GENESYS_VID: u16 = 0x05E3;
    const GL3590_PID: u16 = 0x0625;

    let devices = match rusb::devices() {
        Ok(devices) => devices,
        Err(_) => return Vec::new(),
    };

    devices
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            let vid = descriptor.vendor_id();
            let pid = descriptor.product_id();
            let matches = (vid == REALTEK_VID && (pid == RTL5432_PID || pid == RTL5424_PID))
                || (vid == GENESYS_VID && pid == GL3590_PID);
            if matches {
                Some(UsbModuleObservation {
                    vendor_id: vid,
                    product_id: pid,
                    slot_index: -1,
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn detect_ethernet_cards_local() -> Vec<UsbModuleObservation> {
    const REALTEK_VID: u16 = 0x0BDA;
    const RTL8156B_PID: u16 = 0x8156;

    let devices = match rusb::devices() {
        Ok(devices) => devices,
        Err(_) => return Vec::new(),
    };

    devices
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            if descriptor.vendor_id() == REALTEK_VID && descriptor.product_id() == RTL8156B_PID {
                Some(UsbModuleObservation {
                    vendor_id: descriptor.vendor_id(),
                    product_id: descriptor.product_id(),
                    slot_index: -1,
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn detect_sd_cards_local() -> Vec<UsbModuleObservation> {
    const GENESYS_VID: u16 = 0x05E3;
    const GL3230_SD_PID: u16 = 0x0749;

    let devices = match rusb::devices() {
        Ok(devices) => devices,
        Err(_) => return Vec::new(),
    };

    devices
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            if descriptor.vendor_id() == GENESYS_VID && descriptor.product_id() == GL3230_SD_PID {
                Some(UsbModuleObservation {
                    vendor_id: descriptor.vendor_id(),
                    product_id: descriptor.product_id(),
                    slot_index: -1,
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn detect_microsd_cards_local() -> Vec<UsbModuleObservation> {
    const GENESYS_VID: u16 = 0x05E3;
    const GL_MICROSD_PID: u16 = 0x0751;

    let devices = match rusb::devices() {
        Ok(devices) => devices,
        Err(_) => return Vec::new(),
    };

    devices
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            if descriptor.vendor_id() == GENESYS_VID && descriptor.product_id() == GL_MICROSD_PID {
                Some(UsbModuleObservation {
                    vendor_id: descriptor.vendor_id(),
                    product_id: descriptor.product_id(),
                    slot_index: -1,
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn detect_audio_cards_local() -> Vec<UsbModuleObservation> {
    let devices = match rusb::devices() {
        Ok(devices) => devices,
        Err(_) => return Vec::new(),
    };

    devices
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            if descriptor.vendor_id() == audio_card::FRAMEWORK_VID
                && descriptor.product_id() == audio_card::AUDIO_CARD_PID
            {
                Some(UsbModuleObservation {
                    vendor_id: descriptor.vendor_id(),
                    product_id: descriptor.product_id(),
                    slot_index: -1,
                })
            } else {
                None
            }
        })
        .collect()
}

fn map_framework16_input_slot(port_numbers: &[u8]) -> i32 {
    match port_numbers {
        [4, 2] => 0,
        [4, 3] => 1,
        [3, 1] => 2,
        [3, 2] => 3,
        [3, 3] => 4,
        _ => -1,
    }
}

pub(super) fn detect_input_modules_local() -> Vec<UsbModuleObservation> {
    let devices = match rusb::devices() {
        Ok(devices) => devices,
        Err(_) => return Vec::new(),
    };

    devices
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            if descriptor.vendor_id() != inputmodule::FRAMEWORK_VID
                || !inputmodule::FRAMEWORK16_INPUTMODULE_PIDS.contains(&descriptor.product_id())
            {
                return None;
            }

            let slot_index = device
                .port_numbers()
                .ok()
                .map(|ports| map_framework16_input_slot(&ports))
                .unwrap_or(-1);

            Some(UsbModuleObservation {
                vendor_id: descriptor.vendor_id(),
                product_id: descriptor.product_id(),
                slot_index,
            })
        })
        .collect()
}

pub(super) fn detect_touchpads_local() -> Vec<HidModuleObservation> {
    const TOUCHPAD_USAGE_PAGE: u16 = 0xFF00;
    const TOUCHPAD_PIDS: [u16; 4] = [0x0274, 0x0239, 0x0360, 0x0343];

    let api = match HidApi::new() {
        Ok(api) => api,
        Err(_) => return Vec::new(),
    };

    api.device_list()
        .filter_map(|device| {
            if device.vendor_id() == touchpad::PIX_VID
                && TOUCHPAD_PIDS.contains(&device.product_id())
                && device.usage_page() == TOUCHPAD_USAGE_PAGE
            {
                Some(HidModuleObservation {
                    vendor_id: device.vendor_id(),
                    product_id: device.product_id(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn detect_touchscreens_local() -> Vec<HidModuleObservation> {
    let api = match HidApi::new() {
        Ok(api) => api,
        Err(_) => return Vec::new(),
    };

    api.device_list()
        .filter_map(|device| {
            let vendor_id = device.vendor_id();
            let product_id = device.product_id();
            let usage_page = device.usage_page();
            let is_ili = vendor_id == touchscreen::ILI_VID
                && product_id == touchscreen::ILI_PID
                && usage_page == 0xFF00;
            let is_hx = vendor_id == touchscreen::HX_VID && product_id == touchscreen::HX_PID;

            if is_ili || is_hx {
                Some(HidModuleObservation {
                    vendor_id,
                    product_id,
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn detect_cameras_local() -> Vec<UsbModuleObservation> {
    let devices = match rusb::devices() {
        Ok(devices) => devices,
        Err(_) => return Vec::new(),
    };

    devices
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            if descriptor.vendor_id() == camera::FRAMEWORK_VID
                && (descriptor.product_id() == camera::FRAMEWORK13_16_2ND_GEN_PID
                    || descriptor.product_id() == camera::FRAMEWORK12_PID)
            {
                Some(UsbModuleObservation {
                    vendor_id: descriptor.vendor_id(),
                    product_id: descriptor.product_id(),
                    slot_index: -1,
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn push_detached_module(
    detached: &mut [FrameworkModuleDescriptor; 4],
    detached_count: &mut u8,
    descriptor: FrameworkModuleDescriptor,
) {
    if let Some(slot) = detached.get_mut(*detached_count as usize) {
        *slot = descriptor;
        *detached_count += 1;
    }
}
