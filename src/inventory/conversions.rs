use framework_lib::ccgx::hid;
use framework_lib::chromium_ec::commands::{
    ExpansionBayBoard, ExpansionBayIssue, FpLedBrightnessLevel, GpuPcieConfig, GpuVendor,
};
use framework_lib::chromium_ec::input_deck::InputModuleType;
use framework_lib::inputmodule;

use crate::*;

pub(super) fn module_flag(flag: FrameworkModuleFlag) -> u32 {
    flag as u32
}

pub(super) fn framework_feature_flag(flag: FrameworkEcFeatureFlag) -> u64 {
    flag as u64
}

pub(crate) fn fingerprint_led_level(
    level: Option<FpLedBrightnessLevel>,
) -> FrameworkFingerprintLedLevel {
    match level {
        Some(FpLedBrightnessLevel::High) => FrameworkFingerprintLedLevel::High,
        Some(FpLedBrightnessLevel::Medium) => FrameworkFingerprintLedLevel::Medium,
        Some(FpLedBrightnessLevel::Low) => FrameworkFingerprintLedLevel::Low,
        Some(FpLedBrightnessLevel::UltraLow) => FrameworkFingerprintLedLevel::UltraLow,
        Some(FpLedBrightnessLevel::Custom) => FrameworkFingerprintLedLevel::Custom,
        Some(FpLedBrightnessLevel::Auto) => FrameworkFingerprintLedLevel::Auto,
        None => FrameworkFingerprintLedLevel::Unknown,
    }
}

pub(super) fn expansion_bay_board(
    board: Result<ExpansionBayBoard, ExpansionBayIssue>,
) -> FrameworkExpansionBayBoard {
    match board {
        Ok(ExpansionBayBoard::DualInterposer) => FrameworkExpansionBayBoard::DualInterposer,
        Ok(ExpansionBayBoard::SingleInterposer) => FrameworkExpansionBayBoard::SingleInterposer,
        Ok(ExpansionBayBoard::UmaFans) => FrameworkExpansionBayBoard::UmaFans,
        Err(ExpansionBayIssue::NoModule) => FrameworkExpansionBayBoard::NoModule,
        Err(ExpansionBayIssue::BadConnection(_, _)) => FrameworkExpansionBayBoard::BadConnection,
    }
}

pub(super) fn expansion_bay_vendor(vendor: Option<GpuVendor>) -> FrameworkExpansionBayVendor {
    match vendor {
        Some(GpuVendor::Initializing) => FrameworkExpansionBayVendor::Initializing,
        Some(GpuVendor::FanOnly) => FrameworkExpansionBayVendor::FanOnly,
        Some(GpuVendor::GpuAmdR23M) => FrameworkExpansionBayVendor::AmdGpu,
        Some(GpuVendor::SsdHolder) => FrameworkExpansionBayVendor::SsdHolder,
        Some(GpuVendor::PcieAccessory) => FrameworkExpansionBayVendor::PcieAccessory,
        Some(GpuVendor::NvidiaGn22) => FrameworkExpansionBayVendor::NvidiaGpu,
        None => FrameworkExpansionBayVendor::Unknown,
    }
}

pub(super) fn gpu_pcie_config(config: Option<GpuPcieConfig>) -> FrameworkGpuPcieConfig {
    match config {
        Some(GpuPcieConfig::Pcie8x1) => FrameworkGpuPcieConfig::Unknown,
        Some(GpuPcieConfig::Pcie4x1) => FrameworkGpuPcieConfig::Pcie4x1,
        Some(GpuPcieConfig::Pcie4x2) => FrameworkGpuPcieConfig::Pcie4x2,
        None => FrameworkGpuPcieConfig::Unknown,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn module_descriptor(
    identity: FrameworkModuleIdentity,
    bus: FrameworkModuleBus,
    slot_kind: FrameworkModuleSlotKind,
    confidence: FrameworkModuleConfidence,
    present: bool,
    slot_index: i32,
    flags: u32,
    vendor_id: u32,
    product_id: u32,
    board_id: i32,
) -> FrameworkModuleDescriptor {
    FrameworkModuleDescriptor {
        identity,
        bus,
        slot_kind,
        confidence,
        present: u8::from(present),
        reserved_0: [0; 3],
        slot_index,
        flags,
        vendor_id,
        product_id,
        board_id,
    }
}

pub(super) fn expansion_card_identity(product_id: u16) -> FrameworkModuleIdentity {
    match product_id {
        hid::DP_CARD_PID => FrameworkModuleIdentity::DpExpansionCard,
        hid::HDMI_CARD_PID => FrameworkModuleIdentity::HdmiExpansionCard,
        _ => FrameworkModuleIdentity::UnknownUsbCOccupant,
    }
}

pub(super) fn expansion_card_type(identity: FrameworkModuleIdentity) -> FrameworkExpansionCardType {
    match identity {
        FrameworkModuleIdentity::DpExpansionCard => FrameworkExpansionCardType::DisplayPort,
        FrameworkModuleIdentity::HdmiExpansionCard => FrameworkExpansionCardType::Hdmi,
        FrameworkModuleIdentity::AudioExpansionCard => FrameworkExpansionCardType::Audio,
        FrameworkModuleIdentity::UsbAExpansionCard => FrameworkExpansionCardType::UsbA,
        FrameworkModuleIdentity::UsbCExpansionCard => FrameworkExpansionCardType::UsbC,
        FrameworkModuleIdentity::EthernetExpansionCard => FrameworkExpansionCardType::Ethernet,
        FrameworkModuleIdentity::Ethernet10GExpansionCard => FrameworkExpansionCardType::Ethernet10G,
        FrameworkModuleIdentity::MicroSdExpansionCard => FrameworkExpansionCardType::MicroSd,
        FrameworkModuleIdentity::SdExpansionCard => FrameworkExpansionCardType::Sd,
        FrameworkModuleIdentity::SsdExpansionCard => FrameworkExpansionCardType::Ssd,
        _ => FrameworkExpansionCardType::Unknown,
    }
}

pub(super) fn framework16_top_row_identity(product_id: u16) -> FrameworkModuleIdentity {
    if product_id == inputmodule::LEDMATRIX_PID {
        FrameworkModuleIdentity::Framework16LedMatrix
    } else {
        FrameworkModuleIdentity::Framework16KeyboardModule
    }
}

pub(super) fn input_deck_module_identity(module_type: InputModuleType) -> FrameworkModuleIdentity {
    match module_type {
        InputModuleType::KeyboardA
        | InputModuleType::KeyboardB
        | InputModuleType::FullWidth
        | InputModuleType::GenericA
        | InputModuleType::GenericB
        | InputModuleType::GenericC
        | InputModuleType::Short
        | InputModuleType::Reserved1
        | InputModuleType::Reserved2
        | InputModuleType::Reserved3
        | InputModuleType::Reserved4
        | InputModuleType::Reserved5
        | InputModuleType::Reserved15 => FrameworkModuleIdentity::Framework16KeyboardModule,
        InputModuleType::HubBoard | InputModuleType::Touchpad | InputModuleType::Disconnected => {
            FrameworkModuleIdentity::Framework16KeyboardModule
        }
    }
}

pub(super) fn expansion_bay_identity(
    board: FrameworkExpansionBayBoard,
    vendor: FrameworkExpansionBayVendor,
) -> FrameworkModuleIdentity {
    match vendor {
        FrameworkExpansionBayVendor::FanOnly => FrameworkModuleIdentity::ExpansionBayFanOnly,
        FrameworkExpansionBayVendor::SsdHolder => FrameworkModuleIdentity::ExpansionBaySsdHolder,
        FrameworkExpansionBayVendor::PcieAccessory => {
            FrameworkModuleIdentity::ExpansionBayPcieAccessory
        }
        FrameworkExpansionBayVendor::AmdGpu => FrameworkModuleIdentity::ExpansionBayAmdGpu,
        FrameworkExpansionBayVendor::NvidiaGpu => FrameworkModuleIdentity::ExpansionBayNvidiaGpu,
        FrameworkExpansionBayVendor::Unknown | FrameworkExpansionBayVendor::Initializing => {
            match board {
                FrameworkExpansionBayBoard::DualInterposer => {
                    FrameworkModuleIdentity::ExpansionBayDualInterposer
                }
                FrameworkExpansionBayBoard::SingleInterposer => {
                    FrameworkModuleIdentity::ExpansionBaySingleInterposer
                }
                FrameworkExpansionBayBoard::UmaFans => FrameworkModuleIdentity::ExpansionBayUmaFans,
                _ => FrameworkModuleIdentity::ExpansionBay,
            }
        }
    }
}
