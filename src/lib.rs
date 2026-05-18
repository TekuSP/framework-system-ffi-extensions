use framework_lib::chromium_ec::CrosEc;
use framework_lib::power;
use framework_lib::smbios;

mod abi_impls;
mod byte_buffer;
mod inventory;
mod results;
mod runtime;
mod status;
mod thermal;

use results::{
    active_driver_result, build_info_result, default_ec_flash_versions, fan_capabilities_result,
    flash_versions_result, platform_family_result, platform_result, power_snapshot_result,
    product_name_result, restore_auto_fan_control_result, set_fan_duty_result, set_fan_rpm_result,
    status_description_result, status_device_error_message_result, thermal_snapshot_result,
};
use runtime::{parse_optional_fan_index, parse_optional_fan_index_u8, require_handle};
use status::{get_device_error_message, status_description};

pub(crate) use runtime::feature_enabled;
pub(crate) use status::status_from_error;

pub struct FrameworkEcHandle {
    ec: CrosEc,
    driver: FrameworkEcDriver,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkStatusCode {
    Success = 0,
    NullPointer = -1,
    InvalidArgument = -2,
    NoDriverAvailable = -3,
    UnsupportedDriver = -4,
    DeviceError = -5,
    EcResponse = -6,
    UnknownResponseCode = -7,
    DataUnavailable = -8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusNoPayload {
    pub reserved: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusInvalidFanIndexRecord {
    pub fan_index: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusEcResponseRecord {
    pub response: FrameworkEcResponseDetail,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusUnknownEcResponseCodeRecord {
    pub response_code: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusDeviceErrorRecord {
    pub message_token: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union FrameworkStatusPayload {
    pub none: FrameworkStatusNoPayload,
    pub invalid_fan_index: FrameworkStatusInvalidFanIndexRecord,
    pub ec_response: FrameworkStatusEcResponseRecord,
    pub unknown_ec_response_code: FrameworkStatusUnknownEcResponseCodeRecord,
    pub device_error: FrameworkStatusDeviceErrorRecord,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkStatus {
    pub code: FrameworkStatusCode,
    pub payload: FrameworkStatusPayload,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcResponseDetail {
    Unknown = -1,
    Success = 0,
    InvalidCommand = 1,
    Error = 2,
    InvalidParameter = 3,
    AccessDenied = 4,
    InvalidResponse = 5,
    InvalidVersion = 6,
    InvalidChecksum = 7,
    InProgress = 8,
    Unavailable = 9,
    Timeout = 10,
    Overflow = 11,
    InvalidHeader = 12,
    RequestTruncated = 13,
    ResponseTooBig = 14,
    BusError = 15,
    Busy = 16,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcDriver {
    Unknown = -1,
    Portio = 0,
    CrosEc = 1,
    Windows = 2,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPlatform {
    Framework12IntelGen13 = 0,
    IntelGen11 = 1,
    IntelGen12 = 2,
    IntelGen13 = 3,
    IntelCoreUltra1 = 4,
    Framework13Amd7080 = 5,
    Framework13AmdAi300 = 6,
    Framework16Amd7080 = 7,
    Framework16AmdAi300 = 8,
    FrameworkDesktopAmdAiMax300 = 9,
    GenericFramework = 10,
    UnknownSystem = 11,
    IntelCoreUltra3 = 12,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPlatformFamily {
    Unknown = -1,
    Framework12 = 0,
    Framework13 = 1,
    Framework16 = 2,
    FrameworkDesktop = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkPlatformResult {
    pub status: FrameworkStatus,
    pub platform: FrameworkPlatform,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkPlatformFamilyResult {
    pub status: FrameworkStatus,
    pub family: FrameworkPlatformFamily,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkTemperatureState {
    Ok = 0,
    NotPresent = 1,
    Error = 2,
    NotPowered = 3,
    NotCalibrated = 4,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcCurrentImage {
    Unknown = 0,
    Ro = 1,
    Rw = 2,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkTemperatureReading {
    pub state: FrameworkTemperatureState,
    pub celsius: i16,
    pub reserved: u16,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkFanState {
    Ok = 0,
    NotPresent = 1,
    Stalled = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkFanFeaturesState {
    None = 0,
    FanControl = 1,
    ThermalReporting = 2,
    All = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkFanReading {
    pub state: FrameworkFanState,
    pub rpm: u16,
    pub reserved: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkThermalSnapshot {
    pub fan_count: u8,
    pub reserved: [u8; 3],
    pub temperature_0: FrameworkTemperatureReading,
    pub temperature_1: FrameworkTemperatureReading,
    pub temperature_2: FrameworkTemperatureReading,
    pub temperature_3: FrameworkTemperatureReading,
    pub temperature_4: FrameworkTemperatureReading,
    pub temperature_5: FrameworkTemperatureReading,
    pub temperature_6: FrameworkTemperatureReading,
    pub temperature_7: FrameworkTemperatureReading,
    pub fan_0: FrameworkFanReading,
    pub fan_1: FrameworkFanReading,
    pub fan_2: FrameworkFanReading,
    pub fan_3: FrameworkFanReading,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkFanCapabilities {
    pub fan_count: u8,
    pub features: FrameworkFanFeaturesState,
    pub reserved: [u8; 2],
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPowerSourceState {
    None = 0,
    AcOnly = 1,
    BatteryOnly = 2,
    AcAndBattery = 3,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkBatteryState {
    NotPresent = 0,
    Idle = 1,
    Charging = 2,
    Discharging = 3,
    ChargingAndDischarging = 4,
    Critical = 5,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkBatterySnapshot {
    pub battery_state: FrameworkBatteryState,
    pub reserved: [u8; 3],
    pub present_voltage: u32,
    pub present_rate: u32,
    pub remaining_capacity: u32,
    pub design_capacity: u32,
    pub design_voltage: u32,
    pub last_full_charge_capacity: u32,
    pub cycle_count: u32,
    pub charge_percentage: u32,
    pub manufacturer: FrameworkByteBuffer,
    pub model_number: FrameworkByteBuffer,
    pub serial_number: FrameworkByteBuffer,
    pub battery_type: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkPowerSnapshot {
    pub power_source_state: FrameworkPowerSourceState,
    pub battery_count: u8,
    pub reserved: [u8; 2],
    pub battery_0: FrameworkBatterySnapshot,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkEcFlashVersions {
    pub current_image: FrameworkEcCurrentImage,
    pub ro_version: FrameworkByteBuffer,
    pub rw_version: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkByteBuffer {
    pub ptr: *mut u8,
    pub length: i32,
    pub capacity: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcHandleResult {
    pub status: FrameworkStatus,
    pub handle: *mut FrameworkEcHandle,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkProductNameResult {
    pub status: FrameworkStatus,
    pub product_name: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcBuildInfoResult {
    pub status: FrameworkStatus,
    pub build_info: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcFlashVersionsResult {
    pub status: FrameworkStatus,
    pub versions: FrameworkEcFlashVersions,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcPowerSnapshotResult {
    pub status: FrameworkStatus,
    pub snapshot: FrameworkPowerSnapshot,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcFanCapabilitiesResult {
    pub status: FrameworkStatus,
    pub capabilities: FrameworkFanCapabilities,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcThermalSnapshotResult {
    pub status: FrameworkStatus,
    pub snapshot: FrameworkThermalSnapshot,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcActiveDriverResult {
    pub status: FrameworkStatus,
    pub driver: FrameworkEcDriver,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkStatusDeviceErrorMessageResult {
    pub status: FrameworkStatus,
    pub message: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkStatusDescriptionResult {
    pub status: FrameworkStatus,
    pub description: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcSetFanRpmResult {
    pub status: FrameworkStatus,
    pub fan_index: i32,
    pub rpm: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcSetFanDutyResult {
    pub status: FrameworkStatus,
    pub fan_index: i32,
    pub percent: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcRestoreAutoFanControlResult {
    pub status: FrameworkStatus,
    pub fan_index: i32,
}

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcFeatureFlag {
    None = 0,
    Keyboard = 1 << 0,
    KeyboardBacklight = 1 << 1,
    Touchpad = 1 << 2,
    Fingerprint = 1 << 3,
    AmbientLight = 1 << 4,
    TabletMode = 1 << 5,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkFingerprintLedLevel {
    Unknown = -1,
    High = 0,
    Medium = 1,
    Low = 2,
    UltraLow = 3,
    Custom = 0xFE,
    Auto = 0xFF,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkExpansionBayBoard {
    Unknown = 0,
    DualInterposer = 1,
    SingleInterposer = 2,
    UmaFans = 3,
    NoModule = 4,
    BadConnection = 5,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkExpansionBayVendor {
    Unknown = 0,
    Initializing = 1,
    FanOnly = 2,
    SsdHolder = 3,
    PcieAccessory = 4,
    AmdGpu = 5,
    NvidiaGpu = 6,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkGpuPcieConfig {
    Unknown = 0,
    Pcie4x1 = 1,
    Pcie4x2 = 2,
    Pcie4x4 = 3,
    Pcie5x4 = 4,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkModuleIdentity {
    None = 0,
    UnknownUsbCOccupant = 1,
    DpExpansionCard = 2,
    HdmiExpansionCard = 3,
    AudioExpansionCard = 4,
    Framework16KeyboardModule = 5,
    Framework16LedMatrix = 6,
    Framework16TouchpadModule = 7,
    InternalKeyboard = 8,
    InternalTouchpad = 9,
    FingerprintReader = 10,
    Touchscreen = 11,
    Webcam = 12,
    ExpansionBay = 13,
    ExpansionBayDualInterposer = 14,
    ExpansionBaySingleInterposer = 15,
    ExpansionBayUmaFans = 16,
    ExpansionBaySsdHolder = 17,
    ExpansionBayPcieAccessory = 18,
    ExpansionBayAmdGpu = 19,
    ExpansionBayNvidiaGpu = 20,
    ExpansionBayFanOnly = 21,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkModuleBus {
    Unknown = 0,
    Ec = 1,
    Usb = 2,
    Hid = 3,
    Composite = 4,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkModuleSlotKind {
    None = 0,
    UsbCPort = 1,
    InputDeckTopRow = 2,
    InputDeckTouchpad = 3,
    ExpansionBay = 4,
    InternalFixed = 5,
    Detached = 6,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkModuleConfidence {
    Unknown = 0,
    DerivedWeak = 1,
    DerivedStrong = 2,
    Direct = 3,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkModuleFlag {
    BuiltIn = 1 << 0,
    Active = 1 << 1,
    Connected = 1 << 2,
    Fault = 1 << 3,
    Ambiguous = 1 << 4,
    HasPdContract = 1 << 5,
    DisplayAltMode = 1 << 6,
    DoorClosed = 1 << 7,
    Enabled = 1 << 8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcFeatureFlagsResult {
    pub status: FrameworkStatus,
    pub flags: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcKeyboardBacklightResult {
    pub status: FrameworkStatus,
    pub brightness_percent: u8,
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkEcFingerprintLedState {
    pub raw_level: u8,
    pub reserved: [u8; 3],
    pub level: FrameworkFingerprintLedLevel,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcFingerprintLedResult {
    pub status: FrameworkStatus,
    pub state: FrameworkEcFingerprintLedState,
}

#[repr(C)]
#[derive(Clone)]
pub struct FrameworkEcExpansionBayStatus {
    pub present: u8,
    pub enabled: u8,
    pub fault: u8,
    pub door_closed: u8,
    pub board: FrameworkExpansionBayBoard,
    pub vendor: FrameworkExpansionBayVendor,
    pub config: FrameworkGpuPcieConfig,
    pub reserved: [u8; 3],
    pub serial_number: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone)]
pub struct FrameworkEcExpansionBayStatusResult {
    pub status: FrameworkStatus,
    pub bay: FrameworkEcExpansionBayStatus,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkModuleDescriptor {
    pub identity: FrameworkModuleIdentity,
    pub bus: FrameworkModuleBus,
    pub slot_kind: FrameworkModuleSlotKind,
    pub confidence: FrameworkModuleConfidence,
    pub present: u8,
    pub reserved_0: [u8; 3],
    pub slot_index: i32,
    pub flags: u32,
    pub vendor_id: u32,
    pub product_id: u32,
    pub board_id: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkModuleInventory {
    pub usb_c_slot_count: u8,
    pub input_top_row_count: u8,
    pub detached_count: u8,
    pub reserved_0: u8,
    pub usb_c_slot_0: FrameworkModuleDescriptor,
    pub usb_c_slot_1: FrameworkModuleDescriptor,
    pub usb_c_slot_2: FrameworkModuleDescriptor,
    pub usb_c_slot_3: FrameworkModuleDescriptor,
    pub usb_c_slot_4: FrameworkModuleDescriptor,
    pub usb_c_slot_5: FrameworkModuleDescriptor,
    pub input_top_row_0: FrameworkModuleDescriptor,
    pub input_top_row_1: FrameworkModuleDescriptor,
    pub input_top_row_2: FrameworkModuleDescriptor,
    pub input_top_row_3: FrameworkModuleDescriptor,
    pub input_top_row_4: FrameworkModuleDescriptor,
    pub input_touchpad: FrameworkModuleDescriptor,
    pub internal_keyboard: FrameworkModuleDescriptor,
    pub internal_touchpad: FrameworkModuleDescriptor,
    pub fingerprint_reader: FrameworkModuleDescriptor,
    pub touchscreen: FrameworkModuleDescriptor,
    pub webcam: FrameworkModuleDescriptor,
    pub expansion_bay: FrameworkModuleDescriptor,
    pub detached_0: FrameworkModuleDescriptor,
    pub detached_1: FrameworkModuleDescriptor,
    pub detached_2: FrameworkModuleDescriptor,
    pub detached_3: FrameworkModuleDescriptor,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcModuleInventoryResult {
    pub status: FrameworkStatus,
    pub inventory: FrameworkModuleInventory,
}

#[no_mangle]
pub extern "C" fn framework_ec_driver_is_supported(driver: FrameworkEcDriver) -> bool {
    runtime::driver_is_supported(driver)
}

#[no_mangle]
/// The returned `message` buffer must be released with
/// `framework_byte_buffer_free`.
pub extern "C" fn framework_status_get_device_error_message(
    status: FrameworkStatus,
) -> FrameworkStatusDeviceErrorMessageResult {
    let Some(message) = status
        .device_error_message_token()
        .and_then(get_device_error_message)
    else {
        return status_device_error_message_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            FrameworkByteBuffer::default(),
        );
    };

    status_device_error_message_result(
        FrameworkStatus::success(),
        FrameworkByteBuffer::from_vec(message.into_bytes()),
    )
}

#[no_mangle]
/// The returned `description` buffer must be released with
/// `framework_byte_buffer_free`.
pub extern "C" fn framework_status_get_description(
    status: FrameworkStatus,
) -> FrameworkStatusDescriptionResult {
    status_description_result(
        FrameworkStatus::success(),
        FrameworkByteBuffer::from_vec(status_description(status).into_bytes()),
    )
}

#[no_mangle]
pub extern "C" fn framework_ec_open_default() -> FrameworkEcHandleResult {
    runtime::open_default_ec()
}

#[no_mangle]
pub extern "C" fn framework_ec_open_with_driver(
    driver: FrameworkEcDriver,
) -> FrameworkEcHandleResult {
    runtime::open_with_driver_ec(driver)
}

#[no_mangle]
/// # Safety
/// `handle` must either be null or be a pointer previously returned by one of
/// the `framework_ec_open_*` functions that has not already been freed.
pub unsafe extern "C" fn framework_ec_close(handle: *mut FrameworkEcHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_active_driver(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcActiveDriverResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return active_driver_result(status, FrameworkEcDriver::Unknown),
    };

    active_driver_result(FrameworkStatus::success(), handle.driver)
}

#[no_mangle]
pub extern "C" fn framework_get_platform() -> FrameworkPlatformResult {
    match smbios::get_platform() {
        Some(platform) => platform_result(FrameworkStatus::success(), platform.into()),
        None => platform_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            FrameworkPlatform::UnknownSystem,
        ),
    }
}

#[no_mangle]
pub extern "C" fn framework_get_platform_family() -> FrameworkPlatformFamilyResult {
    match smbios::get_family() {
        Some(family) => platform_family_result(FrameworkStatus::success(), family.into()),
        None => platform_family_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            FrameworkPlatformFamily::Unknown,
        ),
    }
}

#[no_mangle]
pub extern "C" fn framework_get_product_name() -> FrameworkProductNameResult {
    let Some(product_name) = smbios::get_product_name() else {
        return product_name_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            FrameworkByteBuffer::default(),
        );
    };

    product_name_result(
        FrameworkStatus::success(),
        FrameworkByteBuffer::from_vec(product_name.into_bytes()),
    )
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library. The returned
/// `build_info` buffer must be released with `framework_byte_buffer_free`.
pub unsafe extern "C" fn framework_ec_get_build_info(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcBuildInfoResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return build_info_result(status, FrameworkByteBuffer::default()),
    };

    match handle.ec.version_info() {
        Ok(build_info) => build_info_result(
            FrameworkStatus::success(),
            FrameworkByteBuffer::from_vec(build_info.into_bytes()),
        ),
        Err(error) => build_info_result(status_from_error(error), FrameworkByteBuffer::default()),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_flash_versions(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcFlashVersionsResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return flash_versions_result(status, default_ec_flash_versions()),
    };

    let Some((ro_version, rw_version, current_image)) = handle.ec.flash_version() else {
        return flash_versions_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            default_ec_flash_versions(),
        );
    };

    flash_versions_result(
        FrameworkStatus::success(),
        FrameworkEcFlashVersions {
            current_image: current_image.into(),
            ro_version: FrameworkByteBuffer::from_vec(ro_version.into_bytes()),
            rw_version: FrameworkByteBuffer::from_vec(rw_version.into_bytes()),
        },
    )
}

#[no_mangle]
/// # Safety
/// `buffer` must either be the default zeroed buffer or a buffer previously
/// returned by this library that has not already been freed.
pub unsafe extern "C" fn framework_byte_buffer_free(buffer: FrameworkByteBuffer) {
    buffer.destroy();
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_power_snapshot(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcPowerSnapshotResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return power_snapshot_result(status, thermal::default_power_snapshot()),
    };

    let Some(power_info) = power::power_info(&handle.ec) else {
        return power_snapshot_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            thermal::default_power_snapshot(),
        );
    };

    let mut snapshot = FrameworkPowerSnapshot {
        power_source_state: thermal::power_source_state(
            power_info.ac_present,
            power_info.battery.is_some(),
        ),
        ..thermal::default_power_snapshot()
    };

    if let Some(battery) = power_info.battery {
        snapshot.battery_count = battery.battery_count;
        snapshot.battery_0 = FrameworkBatterySnapshot {
            battery_state: thermal::battery_state(
                battery.level_critical,
                battery.discharging,
                battery.charging,
            ),
            reserved: [0; 3],
            present_voltage: battery.present_voltage,
            present_rate: battery.present_rate,
            remaining_capacity: battery.remaining_capacity,
            design_capacity: battery.design_capacity,
            design_voltage: battery.design_voltage,
            last_full_charge_capacity: battery.last_full_charge_capacity,
            cycle_count: battery.cycle_count,
            charge_percentage: battery.charge_percentage,
            manufacturer: FrameworkByteBuffer::from_vec(battery.manufacturer.into_bytes()),
            model_number: FrameworkByteBuffer::from_vec(battery.model_number.into_bytes()),
            serial_number: FrameworkByteBuffer::from_vec(battery.serial_number.into_bytes()),
            battery_type: FrameworkByteBuffer::from_vec(battery.battery_type.into_bytes()),
        };
    }

    power_snapshot_result(FrameworkStatus::success(), snapshot)
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_fan_capabilities(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcFanCapabilitiesResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fan_capabilities_result(status, thermal::default_fan_capabilities()),
    };

    match thermal::build_fan_capabilities(&handle.ec) {
        Ok(capabilities) => fan_capabilities_result(FrameworkStatus::success(), capabilities),
        Err(status) => fan_capabilities_result(status, thermal::default_fan_capabilities()),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_thermal_snapshot(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcThermalSnapshotResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return thermal_snapshot_result(status, thermal::default_thermal_snapshot()),
    };

    let Some(snapshot) = thermal::build_thermal_snapshot(&handle.ec) else {
        return thermal_snapshot_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            thermal::default_thermal_snapshot(),
        );
    };

    thermal_snapshot_result(FrameworkStatus::success(), snapshot)
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_feature_flags(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcFeatureFlagsResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            let mut result = inventory::default_feature_flags_result();
            result.status = status;
            return result;
        }
    };

    match inventory::feature_flags(&handle.ec) {
        Ok(flags) => inventory::feature_flags_result(FrameworkStatus::success(), flags),
        Err(status) => inventory::feature_flags_result(status, 0),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_keyboard_backlight(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcKeyboardBacklightResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            let mut result = inventory::default_keyboard_backlight_result();
            result.status = status;
            return result;
        }
    };

    match handle.ec.get_keyboard_backlight() {
        Ok(level) => inventory::keyboard_backlight_result(FrameworkStatus::success(), level),
        Err(error) => inventory::keyboard_backlight_result(status_from_error(error), 0),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_fingerprint_led(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcFingerprintLedResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            let mut result = inventory::default_fingerprint_led_result();
            result.status = status;
            return result;
        }
    };

    match handle.ec.get_fp_led_level() {
        Ok((raw_level, level)) => inventory::fingerprint_led_result(
            FrameworkStatus::success(),
            raw_level,
            inventory::fingerprint_led_level(level),
        ),
        Err(error) => inventory::fingerprint_led_result(
            status_from_error(error),
            0,
            FrameworkFingerprintLedLevel::Unknown,
        ),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_expansion_bay_status(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcExpansionBayStatusResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            let mut result = inventory::default_expansion_bay_status_result();
            result.status = status;
            return result;
        }
    };

    match inventory::expansion_bay_status(&handle.ec) {
        Ok(bay) => inventory::expansion_bay_status_result(FrameworkStatus::success(), bay),
        Err(status) => inventory::expansion_bay_status_result(
            status,
            inventory::default_expansion_bay_status(),
        ),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_module_inventory(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcModuleInventoryResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            let mut result = inventory::default_module_inventory_result();
            result.status = status;
            return result;
        }
    };

    inventory::module_inventory_result(
        FrameworkStatus::success(),
        inventory::build_module_inventory(handle),
    )
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_set_fan_rpm(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
    rpm: u32,
) -> FrameworkEcSetFanRpmResult {
    let requested_fan_index = fan_index;

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return set_fan_rpm_result(status, fan_index, rpm),
    };
    let fan_index = match parse_optional_fan_index(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return set_fan_rpm_result(status, requested_fan_index, rpm),
    };

    let status = match handle.ec.fan_set_rpm(fan_index, rpm) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    };

    set_fan_rpm_result(status, requested_fan_index, rpm)
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_set_fan_duty(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
    percent: u32,
) -> FrameworkEcSetFanDutyResult {
    let requested_fan_index = fan_index;

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return set_fan_duty_result(status, fan_index, percent),
    };
    let fan_index = match parse_optional_fan_index(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return set_fan_duty_result(status, requested_fan_index, percent),
    };

    let status = match handle.ec.fan_set_duty(fan_index, percent) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    };

    set_fan_duty_result(status, requested_fan_index, percent)
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_restore_auto_fan_control(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
) -> FrameworkEcRestoreAutoFanControlResult {
    let requested_fan_index = fan_index;

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return restore_auto_fan_control_result(status, fan_index),
    };
    let fan_index = match parse_optional_fan_index_u8(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return restore_auto_fan_control_result(status, requested_fan_index),
    };

    let status = match handle.ec.autofanctrl(fan_index) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    };

    restore_auto_fan_control_result(status, requested_fan_index)
}
