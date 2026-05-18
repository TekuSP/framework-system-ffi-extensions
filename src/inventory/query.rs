use framework_lib::chromium_ec::commands::{
    EcFeatureCode, EcRequestExpansionBayStatus, EcRequestGetGpuPcie, GpuPcieConfig, GpuVendor,
};
use framework_lib::chromium_ec::{CrosEc, EcRequestRaw};

use crate::*;

use super::conversions::{
    expansion_bay_board, expansion_bay_vendor, framework_feature_flag, gpu_pcie_config,
};

pub(crate) fn expansion_bay_status(
    ec: &CrosEc,
) -> Result<FrameworkEcExpansionBayStatus, FrameworkStatus> {
    let info = EcRequestExpansionBayStatus {}
        .send_command(ec)
        .map_err(crate::status_from_error)?;
    let gpu = EcRequestGetGpuPcie {}
        .send_command(ec)
        .map_err(crate::status_from_error)?;

    let board = expansion_bay_board(info.expansion_bay_board());
    let vendor = expansion_bay_vendor(match gpu.gpu_vendor {
        0x00 => Some(GpuVendor::Initializing),
        0x01 => Some(GpuVendor::FanOnly),
        0x02 => Some(GpuVendor::GpuAmdR23M),
        0x03 => Some(GpuVendor::SsdHolder),
        0x04 => Some(GpuVendor::PcieAccessory),
        0x05 => Some(GpuVendor::NvidiaGn22),
        _ => None,
    });
    let config = gpu_pcie_config(match gpu.gpu_pcie_config {
        0 => Some(GpuPcieConfig::Pcie8x1),
        1 => Some(GpuPcieConfig::Pcie4x1),
        2 => Some(GpuPcieConfig::Pcie4x2),
        _ => None,
    });
    let present = !matches!(
        board,
        FrameworkExpansionBayBoard::NoModule | FrameworkExpansionBayBoard::Unknown
    ) || matches!(
        vendor,
        FrameworkExpansionBayVendor::FanOnly
            | FrameworkExpansionBayVendor::SsdHolder
            | FrameworkExpansionBayVendor::PcieAccessory
            | FrameworkExpansionBayVendor::AmdGpu
            | FrameworkExpansionBayVendor::NvidiaGpu
    );

    let serial_number = ec
        .get_gpu_serial()
        .map(|serial| FrameworkByteBuffer::from_vec(serial.into_bytes()))
        .unwrap_or_default();

    Ok(FrameworkEcExpansionBayStatus {
        present: u8::from(present),
        enabled: u8::from(info.module_enabled()),
        fault: u8::from(info.module_fault()),
        door_closed: u8::from(info.hatch_switch_closed()),
        board,
        vendor,
        config,
        reserved: [0; 3],
        serial_number,
    })
}

pub(crate) fn feature_flags(ec: &CrosEc) -> Result<u64, FrameworkStatus> {
    let mut flags = 0u64;

    if crate::feature_enabled(ec, EcFeatureCode::Keyboard)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::Keyboard);
    }
    if crate::feature_enabled(ec, EcFeatureCode::PwmKeyboardBacklight)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::KeyboardBacklight);
    }
    if crate::feature_enabled(ec, EcFeatureCode::Touchpad)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::Touchpad);
    }
    if crate::feature_enabled(ec, EcFeatureCode::Fingerprint)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::Fingerprint);
    }
    if crate::feature_enabled(ec, EcFeatureCode::MotionSense)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::AmbientLight);
    }
    if crate::feature_enabled(ec, EcFeatureCode::MotionSense)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::TabletMode);
    }

    Ok(flags)
}
