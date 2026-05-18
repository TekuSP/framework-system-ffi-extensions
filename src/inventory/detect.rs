use framework_lib::audio_card;
use framework_lib::camera;
use framework_lib::ccgx::hid;
use framework_lib::chromium_ec::commands::EcRequestGetPdPortState;
use framework_lib::chromium_ec::{CrosEc, EcRequestRaw};
use framework_lib::inputmodule;
use framework_lib::touchpad;
use framework_lib::touchscreen;
use hidapi::HidApi;

use crate::*;

use super::conversions::{module_descriptor, module_flag};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PdPortObservation {
    pub(super) connected: bool,
    pub(super) has_pd_contract: bool,
    pub(super) dp_alt_mode: bool,
    pub(super) active: bool,
}

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

pub(super) fn read_pd_port_observation(ec: &CrosEc, port: u8) -> Option<PdPortObservation> {
    let response = EcRequestGetPdPortState { port }.send_command(ec).ok()?;
    let connected = response.c_state != 0;
    let has_pd_contract = response.pd_state != 0;
    let dp_alt_mode = response.pd_alt_mode_status != 0;
    let active = response.active_port != 0 || dp_alt_mode;

    Some(PdPortObservation {
        connected,
        has_pd_contract,
        dp_alt_mode,
        active,
    })
}

fn pd_observation_flags(observation: PdPortObservation) -> u32 {
    let mut flags = 0u32;
    if observation.connected {
        flags |= module_flag(FrameworkModuleFlag::Connected);
    }
    if observation.has_pd_contract {
        flags |= module_flag(FrameworkModuleFlag::HasPdContract);
    }
    if observation.dp_alt_mode {
        flags |= module_flag(FrameworkModuleFlag::DisplayAltMode);
    }
    if observation.active {
        flags |= module_flag(FrameworkModuleFlag::Active);
    }
    flags
}

pub(super) fn unknown_usb_c_descriptor(
    slot_index: usize,
    observation: PdPortObservation,
) -> Option<FrameworkModuleDescriptor> {
    if !observation.connected {
        return None;
    }

    Some(module_descriptor(
        FrameworkModuleIdentity::UnknownUsbCOccupant,
        FrameworkModuleBus::Ec,
        FrameworkModuleSlotKind::UsbCPort,
        if observation.dp_alt_mode {
            FrameworkModuleConfidence::DerivedStrong
        } else {
            FrameworkModuleConfidence::DerivedWeak
        },
        true,
        slot_index as i32,
        pd_observation_flags(observation),
        0,
        0,
        -1,
    ))
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
