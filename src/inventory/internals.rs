use framework_lib::chromium_ec::commands::{BoardIdType, EcFeatureCode};
use framework_lib::chromium_ec::input_deck::InputModuleType;
use framework_lib::smbios::PlatformFamily;

use crate::*;

use super::conversions::{
    framework16_top_row_identity, input_deck_module_identity, module_descriptor, module_flag,
};
use super::defaults::default_module_descriptor;
use super::detect::{
    detect_cameras_local, detect_input_modules_local, detect_touchpads_local,
    detect_touchscreens_local,
};

pub(super) struct InternalModules {
    pub(super) input_top_row_count: u8,
    pub(super) top_row: [FrameworkModuleDescriptor; 5],
    pub(super) input_touchpad: FrameworkModuleDescriptor,
    pub(super) internal_keyboard: FrameworkModuleDescriptor,
    pub(super) internal_touchpad: FrameworkModuleDescriptor,
    pub(super) fingerprint_reader: FrameworkModuleDescriptor,
    pub(super) touchscreen: FrameworkModuleDescriptor,
    pub(super) webcam: FrameworkModuleDescriptor,
}

pub(super) fn detect_internal_modules(
    handle: &FrameworkEcHandle,
    family: Option<PlatformFamily>,
) -> InternalModules {
    let input_top_row_count = if family == Some(PlatformFamily::Framework16) {
        5
    } else {
        0
    };
    let mut top_row = [default_module_descriptor(); 5];
    let mut input_touchpad = default_module_descriptor();
    let mut internal_keyboard = default_module_descriptor();
    let mut internal_touchpad = default_module_descriptor();
    let mut fingerprint_reader = default_module_descriptor();
    let mut touchscreen_module = default_module_descriptor();
    let mut webcam = default_module_descriptor();

    if input_top_row_count > 0 {
        if let Ok(deck) = handle.ec.get_input_deck_status() {
            for (index, slot) in deck.top_row_to_array().iter().copied().enumerate() {
                top_row[index] = module_descriptor(
                    input_deck_module_identity(slot),
                    FrameworkModuleBus::Ec,
                    FrameworkModuleSlotKind::InputDeckTopRow,
                    FrameworkModuleConfidence::Direct,
                    !matches!(slot, InputModuleType::Disconnected | InputModuleType::Short),
                    index as i32,
                    if !matches!(slot, InputModuleType::Disconnected | InputModuleType::Short) {
                        module_flag(FrameworkModuleFlag::Connected)
                    } else {
                        0
                    },
                    0,
                    0,
                    -1,
                );
            }

            input_touchpad = module_descriptor(
                FrameworkModuleIdentity::Framework16TouchpadModule,
                FrameworkModuleBus::Ec,
                FrameworkModuleSlotKind::InputDeckTouchpad,
                FrameworkModuleConfidence::Direct,
                deck.touchpad_present,
                0,
                if deck.touchpad_present {
                    module_flag(FrameworkModuleFlag::Connected)
                } else {
                    0
                },
                0,
                0,
                i32::from(deck.touchpad_id),
            );
        }

        for module in detect_input_modules_local() {
            if module.slot_index < 0 || module.slot_index >= top_row.len() as i32 {
                continue;
            }
            top_row[module.slot_index as usize] = module_descriptor(
                framework16_top_row_identity(module.product_id),
                FrameworkModuleBus::Usb,
                FrameworkModuleSlotKind::InputDeckTopRow,
                FrameworkModuleConfidence::Direct,
                true,
                module.slot_index,
                module_flag(FrameworkModuleFlag::Connected),
                module.vendor_id as u32,
                module.product_id as u32,
                -1,
            );
        }
    } else if crate::feature_enabled(&handle.ec, EcFeatureCode::Keyboard).unwrap_or(false) {
        internal_keyboard = module_descriptor(
            FrameworkModuleIdentity::InternalKeyboard,
            FrameworkModuleBus::Ec,
            FrameworkModuleSlotKind::InternalFixed,
            FrameworkModuleConfidence::DerivedStrong,
            true,
            -1,
            module_flag(FrameworkModuleFlag::BuiltIn) | module_flag(FrameworkModuleFlag::Connected),
            0,
            0,
            -1,
        );
    }

    let touchpad_devices = detect_touchpads_local();
    if family == Some(PlatformFamily::Framework16) {
        if let Some(device) = touchpad_devices.first() {
            input_touchpad = module_descriptor(
                FrameworkModuleIdentity::Framework16TouchpadModule,
                FrameworkModuleBus::Hid,
                FrameworkModuleSlotKind::InputDeckTouchpad,
                FrameworkModuleConfidence::Direct,
                true,
                0,
                module_flag(FrameworkModuleFlag::Connected),
                device.vendor_id as u32,
                device.product_id as u32,
                input_touchpad.board_id,
            );
        }
    } else {
        let mut present =
            crate::feature_enabled(&handle.ec, EcFeatureCode::Touchpad).unwrap_or(false);
        let mut confidence = if present {
            FrameworkModuleConfidence::DerivedStrong
        } else {
            FrameworkModuleConfidence::Unknown
        };
        let mut vendor_id = 0u32;
        let mut product_id = 0u32;
        let board_id = handle
            .ec
            .read_board_id_hc(BoardIdType::Touchpad)
            .ok()
            .flatten()
            .map(i32::from)
            .unwrap_or(-1);

        if let Some(device) = touchpad_devices.first() {
            present = true;
            confidence = FrameworkModuleConfidence::Direct;
            vendor_id = device.vendor_id as u32;
            product_id = device.product_id as u32;
        }

        if present {
            internal_touchpad = module_descriptor(
                FrameworkModuleIdentity::InternalTouchpad,
                if vendor_id != 0 || product_id != 0 {
                    FrameworkModuleBus::Hid
                } else {
                    FrameworkModuleBus::Ec
                },
                FrameworkModuleSlotKind::InternalFixed,
                confidence,
                true,
                -1,
                module_flag(FrameworkModuleFlag::BuiltIn)
                    | module_flag(FrameworkModuleFlag::Connected),
                vendor_id,
                product_id,
                board_id,
            );
        }
    }

    if crate::feature_enabled(&handle.ec, EcFeatureCode::Fingerprint).unwrap_or(false)
        || handle.ec.get_fp_led_level().is_ok()
    {
        fingerprint_reader = module_descriptor(
            FrameworkModuleIdentity::FingerprintReader,
            FrameworkModuleBus::Ec,
            FrameworkModuleSlotKind::InternalFixed,
            if handle.ec.get_fp_led_level().is_ok() {
                FrameworkModuleConfidence::Direct
            } else {
                FrameworkModuleConfidence::DerivedStrong
            },
            true,
            -1,
            module_flag(FrameworkModuleFlag::BuiltIn) | module_flag(FrameworkModuleFlag::Connected),
            0,
            0,
            -1,
        );
    }

    if let Some(device) = detect_touchscreens_local().first() {
        touchscreen_module = module_descriptor(
            FrameworkModuleIdentity::Touchscreen,
            FrameworkModuleBus::Hid,
            FrameworkModuleSlotKind::InternalFixed,
            FrameworkModuleConfidence::Direct,
            true,
            -1,
            module_flag(FrameworkModuleFlag::BuiltIn) | module_flag(FrameworkModuleFlag::Connected),
            device.vendor_id as u32,
            device.product_id as u32,
            -1,
        );
    }

    if let Some(device) = detect_cameras_local().first() {
        webcam = module_descriptor(
            FrameworkModuleIdentity::Webcam,
            FrameworkModuleBus::Usb,
            FrameworkModuleSlotKind::InternalFixed,
            FrameworkModuleConfidence::Direct,
            true,
            -1,
            module_flag(FrameworkModuleFlag::BuiltIn) | module_flag(FrameworkModuleFlag::Connected),
            device.vendor_id as u32,
            device.product_id as u32,
            -1,
        );
    }

    InternalModules {
        input_top_row_count,
        top_row,
        input_touchpad,
        internal_keyboard,
        internal_touchpad,
        fingerprint_reader,
        touchscreen: touchscreen_module,
        webcam,
    }
}
