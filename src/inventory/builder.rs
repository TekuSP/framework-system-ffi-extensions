use framework_lib::smbios::{self, PlatformFamily};

use crate::*;

use super::conversions::{expansion_bay_identity, module_descriptor, module_flag};
use super::defaults::{default_expansion_card_module_descriptor, default_module_descriptor};
use super::internals::detect_internal_modules;
use super::query::expansion_bay_status;
use super::usb_slots::{populate_usb_slots, MAX_USB_C_SLOT_COUNT};

fn build_expansion_bay_module(handle: &FrameworkEcHandle) -> FrameworkModuleDescriptor {
    let mut expansion_bay_module = default_module_descriptor();

    if let Ok(bay) = expansion_bay_status(&handle.ec) {
        if bay.present != 0 {
            let mut flags = 0u32;
            if bay.enabled != 0 {
                flags |= module_flag(FrameworkModuleFlag::Enabled);
            }
            if bay.fault != 0 {
                flags |= module_flag(FrameworkModuleFlag::Fault);
            }
            if bay.door_closed != 0 {
                flags |= module_flag(FrameworkModuleFlag::DoorClosed);
            }
            flags |= module_flag(FrameworkModuleFlag::Connected);
            expansion_bay_module = module_descriptor(
                expansion_bay_identity(bay.board, bay.vendor),
                FrameworkModuleBus::Ec,
                FrameworkModuleSlotKind::ExpansionBay,
                FrameworkModuleConfidence::Direct,
                true,
                0,
                flags,
                0,
                0,
                -1,
            );
        }
    }

    expansion_bay_module
}

pub(crate) fn build_module_inventory(handle: &FrameworkEcHandle) -> FrameworkModuleInventory {
    let mut usb_slots = [default_expansion_card_module_descriptor(); MAX_USB_C_SLOT_COUNT];
    let mut detached = [default_module_descriptor(); 4];
    let mut detached_count = 0u8;

    let family = smbios::get_family();
    let platform: FrameworkPlatform = smbios::get_platform()
        .map(Into::into)
        .unwrap_or(FrameworkPlatform::UnknownSystem);
    // Expansion-card slot count is platform-specific: FW16 has 6, the Desktop exposes 2 front slots, and the
    // FW12/FW13 laptops have 4. Querying more than exist returns garbage PD state for the non-existent ports.
    let usb_c_slot_count: u8 = match family {
        Some(PlatformFamily::Framework16) => 6,
        Some(PlatformFamily::FrameworkDesktop) => 2,
        _ => 4,
    };

    populate_usb_slots(
        handle,
        usb_c_slot_count,
        platform,
        &mut usb_slots,
        &mut detached,
        &mut detached_count,
    );

    let internals = detect_internal_modules(handle, family);
    let expansion_bay_module = build_expansion_bay_module(handle);

    FrameworkModuleInventory {
        usb_c_slot_count,
        input_top_row_count: internals.input_top_row_count,
        detached_count,
        reserved_0: 0,
        usb_c_slot_0: usb_slots[0],
        usb_c_slot_1: usb_slots[1],
        usb_c_slot_2: usb_slots[2],
        usb_c_slot_3: usb_slots[3],
        usb_c_slot_4: usb_slots[4],
        usb_c_slot_5: usb_slots[5],
        input_top_row_0: internals.top_row[0],
        input_top_row_1: internals.top_row[1],
        input_top_row_2: internals.top_row[2],
        input_top_row_3: internals.top_row[3],
        input_top_row_4: internals.top_row[4],
        input_touchpad: internals.input_touchpad,
        internal_keyboard: internals.internal_keyboard,
        internal_touchpad: internals.internal_touchpad,
        fingerprint_reader: internals.fingerprint_reader,
        touchscreen: internals.touchscreen,
        webcam: internals.webcam,
        expansion_bay: expansion_bay_module,
        detached_0: detached[0],
        detached_1: detached[1],
        detached_2: detached[2],
        detached_3: detached[3],
    }
}
