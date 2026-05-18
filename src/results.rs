use crate::*;

pub(crate) fn default_ec_flash_versions() -> FrameworkEcFlashVersions {
    FrameworkEcFlashVersions {
        current_image: FrameworkEcCurrentImage::Unknown,
        ro_version: FrameworkByteBuffer::default(),
        rw_version: FrameworkByteBuffer::default(),
    }
}

pub(crate) fn ec_handle_result(
    status: FrameworkStatus,
    handle: *mut FrameworkEcHandle,
) -> FrameworkEcHandleResult {
    FrameworkEcHandleResult { status, handle }
}

pub(crate) fn product_name_result(
    status: FrameworkStatus,
    product_name: FrameworkByteBuffer,
) -> FrameworkProductNameResult {
    FrameworkProductNameResult {
        status,
        product_name,
    }
}

pub(crate) fn build_info_result(
    status: FrameworkStatus,
    build_info: FrameworkByteBuffer,
) -> FrameworkEcBuildInfoResult {
    FrameworkEcBuildInfoResult { status, build_info }
}

pub(crate) fn flash_versions_result(
    status: FrameworkStatus,
    versions: FrameworkEcFlashVersions,
) -> FrameworkEcFlashVersionsResult {
    FrameworkEcFlashVersionsResult { status, versions }
}

pub(crate) fn power_snapshot_result(
    status: FrameworkStatus,
    snapshot: FrameworkPowerSnapshot,
) -> FrameworkEcPowerSnapshotResult {
    FrameworkEcPowerSnapshotResult { status, snapshot }
}

pub(crate) fn fan_capabilities_result(
    status: FrameworkStatus,
    capabilities: FrameworkFanCapabilities,
) -> FrameworkEcFanCapabilitiesResult {
    FrameworkEcFanCapabilitiesResult {
        status,
        capabilities,
    }
}

pub(crate) fn thermal_snapshot_result(
    status: FrameworkStatus,
    snapshot: FrameworkThermalSnapshot,
) -> FrameworkEcThermalSnapshotResult {
    FrameworkEcThermalSnapshotResult { status, snapshot }
}

pub(crate) fn active_driver_result(
    status: FrameworkStatus,
    driver: FrameworkEcDriver,
) -> FrameworkEcActiveDriverResult {
    FrameworkEcActiveDriverResult { status, driver }
}

pub(crate) fn status_device_error_message_result(
    status: FrameworkStatus,
    message: FrameworkByteBuffer,
) -> FrameworkStatusDeviceErrorMessageResult {
    FrameworkStatusDeviceErrorMessageResult { status, message }
}

pub(crate) fn status_description_result(
    status: FrameworkStatus,
    description: FrameworkByteBuffer,
) -> FrameworkStatusDescriptionResult {
    FrameworkStatusDescriptionResult {
        status,
        description,
    }
}

pub(crate) fn platform_result(
    status: FrameworkStatus,
    platform: FrameworkPlatform,
) -> FrameworkPlatformResult {
    FrameworkPlatformResult { status, platform }
}

pub(crate) fn platform_family_result(
    status: FrameworkStatus,
    family: FrameworkPlatformFamily,
) -> FrameworkPlatformFamilyResult {
    FrameworkPlatformFamilyResult { status, family }
}

pub(crate) fn set_fan_rpm_result(
    status: FrameworkStatus,
    fan_index: i32,
    rpm: u32,
) -> FrameworkEcSetFanRpmResult {
    FrameworkEcSetFanRpmResult {
        status,
        fan_index,
        rpm,
    }
}

pub(crate) fn set_fan_duty_result(
    status: FrameworkStatus,
    fan_index: i32,
    percent: u32,
) -> FrameworkEcSetFanDutyResult {
    FrameworkEcSetFanDutyResult {
        status,
        fan_index,
        percent,
    }
}

pub(crate) fn restore_auto_fan_control_result(
    status: FrameworkStatus,
    fan_index: i32,
) -> FrameworkEcRestoreAutoFanControlResult {
    FrameworkEcRestoreAutoFanControlResult { status, fan_index }
}
