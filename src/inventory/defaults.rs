use crate::pd;
use crate::*;

pub(crate) fn default_feature_flags_result() -> FrameworkEcFeatureFlagsResult {
    FrameworkEcFeatureFlagsResult {
        status: FrameworkStatus::success(),
        flags: 0,
    }
}

pub(crate) fn default_keyboard_backlight_result() -> FrameworkEcKeyboardBacklightResult {
    FrameworkEcKeyboardBacklightResult {
        status: FrameworkStatus::success(),
        brightness_percent: 0,
        reserved: [0; 3],
    }
}

fn default_fingerprint_led_state() -> FrameworkEcFingerprintLedState {
    FrameworkEcFingerprintLedState {
        raw_level: 0,
        reserved: [0; 3],
        level: FrameworkFingerprintLedLevel::Unknown,
    }
}

pub(crate) fn default_fingerprint_led_result() -> FrameworkEcFingerprintLedResult {
    FrameworkEcFingerprintLedResult {
        status: FrameworkStatus::success(),
        state: default_fingerprint_led_state(),
    }
}

pub(crate) fn default_expansion_bay_status() -> FrameworkEcExpansionBayStatus {
    FrameworkEcExpansionBayStatus {
        present: 0,
        enabled: 0,
        fault: 0,
        door_closed: 0,
        board: FrameworkExpansionBayBoard::Unknown,
        vendor: FrameworkExpansionBayVendor::Unknown,
        config: FrameworkGpuPcieConfig::Unknown,
        reserved: [0; 3],
        serial_number: FrameworkByteBuffer::default(),
    }
}

pub(crate) fn default_expansion_bay_status_result() -> FrameworkEcExpansionBayStatusResult {
    FrameworkEcExpansionBayStatusResult {
        status: FrameworkStatus::success(),
        bay: default_expansion_bay_status(),
    }
}

pub(super) fn default_module_descriptor() -> FrameworkModuleDescriptor {
    FrameworkModuleDescriptor {
        identity: FrameworkModuleIdentity::None,
        bus: FrameworkModuleBus::Unknown,
        slot_kind: FrameworkModuleSlotKind::None,
        confidence: FrameworkModuleConfidence::Unknown,
        present: 0,
        reserved_0: [0; 3],
        slot_index: -1,
        flags: 0,
        vendor_id: 0,
        product_id: 0,
        board_id: -1,
        position: FrameworkInputModulePosition::Unknown,
    }
}

pub(super) fn default_expansion_card_module_descriptor() -> FrameworkExpansionCardModuleDescriptor {
    let d = default_module_descriptor();
    FrameworkExpansionCardModuleDescriptor {
        identity: d.identity,
        bus: d.bus,
        slot_kind: d.slot_kind,
        confidence: d.confidence,
        present: d.present,
        reserved_0: d.reserved_0,
        slot_index: d.slot_index,
        flags: d.flags,
        vendor_id: d.vendor_id,
        product_id: d.product_id,
        board_id: d.board_id,
        pd: pd::default_pd_port_state(),
        card_type: FrameworkExpansionCardType::Unknown,
        card_confidence: FrameworkModuleConfidence::Unknown,
        reserved: 0,
    }
}

fn default_module_inventory() -> FrameworkModuleInventory {
    FrameworkModuleInventory {
        usb_c_slot_count: 0,
        input_top_row_count: 0,
        detached_count: 0,
        reserved_0: 0,
        usb_c_slot_0: default_expansion_card_module_descriptor(),
        usb_c_slot_1: default_expansion_card_module_descriptor(),
        usb_c_slot_2: default_expansion_card_module_descriptor(),
        usb_c_slot_3: default_expansion_card_module_descriptor(),
        usb_c_slot_4: default_expansion_card_module_descriptor(),
        usb_c_slot_5: default_expansion_card_module_descriptor(),
        input_top_row_0: default_module_descriptor(),
        input_top_row_1: default_module_descriptor(),
        input_top_row_2: default_module_descriptor(),
        input_top_row_3: default_module_descriptor(),
        input_top_row_4: default_module_descriptor(),
        input_touchpad: default_module_descriptor(),
        internal_keyboard: default_module_descriptor(),
        internal_touchpad: default_module_descriptor(),
        fingerprint_reader: default_module_descriptor(),
        touchscreen: default_module_descriptor(),
        webcam: default_module_descriptor(),
        expansion_bay: default_module_descriptor(),
        detached_0: default_module_descriptor(),
        detached_1: default_module_descriptor(),
        detached_2: default_module_descriptor(),
        detached_3: default_module_descriptor(),
    }
}

pub(crate) fn default_module_inventory_result() -> FrameworkEcModuleInventoryResult {
    FrameworkEcModuleInventoryResult {
        status: FrameworkStatus::success(),
        inventory: default_module_inventory(),
    }
}

pub(crate) fn feature_flags_result(
    status: FrameworkStatus,
    flags: u64,
) -> FrameworkEcFeatureFlagsResult {
    FrameworkEcFeatureFlagsResult { status, flags }
}

pub(crate) fn keyboard_backlight_result(
    status: FrameworkStatus,
    brightness_percent: u8,
) -> FrameworkEcKeyboardBacklightResult {
    FrameworkEcKeyboardBacklightResult {
        status,
        brightness_percent,
        reserved: [0; 3],
    }
}

pub(crate) fn fingerprint_led_result(
    status: FrameworkStatus,
    raw_level: u8,
    level: FrameworkFingerprintLedLevel,
) -> FrameworkEcFingerprintLedResult {
    FrameworkEcFingerprintLedResult {
        status,
        state: FrameworkEcFingerprintLedState {
            raw_level,
            reserved: [0; 3],
            level,
        },
    }
}

pub(crate) fn expansion_bay_status_result(
    status: FrameworkStatus,
    bay: FrameworkEcExpansionBayStatus,
) -> FrameworkEcExpansionBayStatusResult {
    FrameworkEcExpansionBayStatusResult { status, bay }
}

pub(crate) fn module_inventory_result(
    status: FrameworkStatus,
    inventory: FrameworkModuleInventory,
) -> FrameworkEcModuleInventoryResult {
    FrameworkEcModuleInventoryResult { status, inventory }
}
