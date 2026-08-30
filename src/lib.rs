use framework_lib::chromium_ec::{CrosEc, EcError, EcResponseStatus};
use framework_lib::power;
use framework_lib::smbios;

mod abi_impls;
mod byte_buffer;
mod controls;
mod diagnostics;
mod gpu_descriptor;
mod inventory;
mod pd;
mod results;
mod runtime;
mod smart_battery;
mod status;
mod thermal;

use results::{
    active_driver_result, build_info_result, default_ec_flash_versions, fan_capabilities_result,
    flash_versions_result, gpu_descriptor_header_result, gpu_descriptor_read_result,
    gpu_descriptor_validation_result, platform_family_result, platform_result,
    power_snapshot_result, product_name_result, restore_auto_fan_control_result,
    set_fan_duty_result, set_fan_rpm_result, status_description_result,
    status_device_error_message_result, thermal_snapshot_result,
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

/// Platform-specific role name for a temperature sensor slot, mirrored from `framework_lib`'s
/// per-platform thermal sensor labels (see `power::print_thermal`). Values beyond the labelled
/// sensors fall back to `Generic`; an undetermined platform yields `Unknown`.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkSensorName {
    /// Platform could not be determined; sensor role is indeterminate.
    Unknown = 0,
    /// Platform known but no specific name assigned to this slot.
    Generic = 1,
    F75303Local = 2,
    F75303Cpu = 3,
    F75303Ddr = 4,
    Battery = 5,
    Peci = 6,
    F57397VccGt = 7,
    F75303Skin = 8,
    ChargerIc = 9,
    Apu = 10,
    DgpuVr = 11,
    DgpuVram = 12,
    DgpuAmb = 13,
    DgpuTemp = 14,
    F75303Apu = 15,
    F75303Amb = 16,
    Virtual = 17,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkTemperatureReading {
    pub state: FrameworkTemperatureState,
    pub celsius: i16,
    /// The sensor's platform role name. Occupies the former `reserved` u16 slot, so the struct
    /// layout/size is unchanged for existing managed callers.
    pub name: FrameworkSensorName,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkFanState {
    Ok = 0,
    NotPresent = 1,
    Stalled = 2,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkFanName {
    Unknown = 0,
    Generic = 1,
    ApuFan = 2,
    LeftFan = 3,
    RightFan = 4,
    FrontFan = 5,
    ThirdFan = 6,
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
    pub name: FrameworkFanName,
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
    UsbAExpansionCard = 22,
    UsbCExpansionCard = 23,
    EthernetExpansionCard = 24,
    Ethernet10GExpansionCard = 25,
    MicroSdExpansionCard = 26,
    SdExpansionCard = 27,
    SsdExpansionCard = 28,
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
    UsbCExpansionCardSlot = 7,
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
    /// 1 when the installed bay module exposes a USB-C port (e.g. the FW16 graphics modules); then `pd` and
    /// `capability` describe that port.
    pub has_usb_c_port: u8,
    pub reserved: [u8; 3],
    pub board: FrameworkExpansionBayBoard,
    pub vendor: FrameworkExpansionBayVendor,
    pub config: FrameworkGpuPcieConfig,
    /// Live Power Delivery state of the bay module's USB-C port (default/empty when `has_usb_c_port == 0`).
    pub pd: FrameworkEcPdPortState,
    /// Static capability of the bay module's USB-C port (default when `has_usb_c_port == 0`).
    pub capability: FrameworkUsbCPortCapability,
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
pub struct FrameworkGpuDescriptorHeader {
    pub magic: [u8; 4],
    pub length: u32,
    pub desc_ver_major: u16,
    pub desc_ver_minor: u16,
    pub hardware_version: u16,
    pub hardware_revision: u16,
    pub serial: [u8; 20],
    pub descriptor_length: u32,
    pub descriptor_crc32: u32,
    pub crc32: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcGpuDescriptorHeaderResult {
    pub status: FrameworkStatus,
    pub header: FrameworkGpuDescriptorHeader,
}

#[repr(C)]
#[derive(Clone)]
pub struct FrameworkEcGpuDescriptorReadResult {
    pub status: FrameworkStatus,
    pub descriptor: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcGpuDescriptorValidationResult {
    pub status: FrameworkStatus,
    pub is_match: u8,
    pub reserved: [u8; 3],
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkExpansionCardType {
    Unknown = 0,
    DisplayPort = 1,
    Hdmi = 2,
    Audio = 3,
    UsbA = 4,
    UsbC = 5,
    Ethernet = 6,
    Ethernet10G = 7,
    MicroSd = 8,
    Sd = 9,
    Ssd = 10,
}

/// USB-C data-lane capability of an expansion-card slot. Static board spec sourced from a per-platform table,
/// not the live negotiated link — the EC does not report this.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkUsbCDataLane {
    Unknown = 0,
    Usb2 = 1,
    Usb32 = 2,
    Usb32Gen2x1 = 3,
    Usb32Gen2x2 = 4,
    Usb4 = 5,
    Thunderbolt4 = 6,
}

/// DisplayPort alt-mode capability and version of an expansion-card slot. Static board spec, not the live
/// alt-mode reported in `FrameworkEcPdPortState::alt_mode_flags`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkDisplayPortCapability {
    None = 0,
    /// DisplayPort 1.4 / 1.4a (HBR3).
    Dp14Hbr3 = 1,
    /// DisplayPort 2.0 without a documented UHBR qualifier (e.g. FW13 Core Ultra Series 1).
    Dp20 = 2,
    Dp20Uhbr10 = 3,
    Dp20Uhbr20 = 4,
    /// DisplayPort 2.1 without a documented UHBR qualifier (e.g. the FW16 GPU-module port).
    Dp21 = 5,
    Dp21Uhbr10 = 6,
    Dp21Uhbr20 = 7,
    /// Supported but the version is not documented in the source table.
    Supported = 8,
}

/// Physical position of an EC PD port, mirroring upstream framework-system (power.rs `get_and_print_pd_info`):
/// index 0 = Right Back, 1 = Right Middle/Front, 2 = Left Middle/Front, 3 = Left Back — "Middle" on Framework 16,
/// "Front" on the other laptops. `GraphicsModule` is the FW16 dGPU rear port; `Unknown` for undocumented platforms.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkUsbCPortPosition {
    Unknown = 0,
    RightBack = 1,
    RightMiddle = 2,
    RightFront = 3,
    LeftMiddle = 4,
    LeftFront = 5,
    LeftBack = 6,
    GraphicsModule = 7,
}

/// Static USB-C port capability of an expansion-card slot, sourced from a per-platform board table (the EC does
/// not expose these). Distinct from the live `FrameworkEcPdPortState` negotiation on the same slot.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkUsbCPortCapability {
    /// 1 when the per-platform table covers this slot; 0 for unknown platforms/slots.
    pub known: u8,
    /// 1 when the slot supports USB Power Delivery charging; 0 for a power-limited (e.g. 900 mA) slot.
    pub supports_pd: u8,
    /// 1 when the "higher power consumption" USB-A note applies to this slot.
    pub usb_a_high_power: u8,
    /// Physical position of this PD port (Right Back, Left Middle, Graphics module, …).
    pub position: FrameworkUsbCPortPosition,
    pub data_lane: FrameworkUsbCDataLane,
    pub displayport: FrameworkDisplayPortCapability,
    /// Maximum charge power in watts (0 when the slot is not a charging port); e.g. 240 or 140.
    pub max_charge_watts: u16,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPdTypeCState {
    Nothing = 0,
    Sink = 1,
    Source = 2,
    Debug = 3,
    Audio = 4,
    PoweredAccessory = 5,
    Unsupported = 6,
    Invalid = 7,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPdPowerRole {
    Sink = 0,
    Source = 1,
    Unknown = 2,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPdDataRole {
    Ufp = 0,
    Dfp = 1,
    Disconnected = 2,
    Unknown = 3,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPdCcPolarity {
    Unknown = -1,
    Cc1 = 0,
    Cc2 = 1,
    Cc1Debug = 2,
    Cc2Debug = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkEcPdPortState {
    pub c_state: FrameworkPdTypeCState,
    pub power_role: FrameworkPdPowerRole,
    pub data_role: FrameworkPdDataRole,
    pub cc_polarity: FrameworkPdCcPolarity,
    pub voltage_mv: u16,
    pub current_ma: u16,
    pub has_pd_contract: u8,
    pub vconn_active: u8,
    pub epr_active: u8,
    pub epr_support: u8,
    pub active_port: u8,
    pub alt_mode_flags: u8,
    pub reserved: [u8; 2],
}

/// Physical position of an input-deck module on Framework Laptop 16 (the 8-wide input-deck MUX). Mirrors the
/// native <c>chromium_ec::input_deck::InputDeckMux</c> slots; <c>Unknown</c> for any module that is not
/// input-deck-mounted or on platforms that do not report a deck position.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkInputModulePosition {
    /// Not an input-deck-mounted module, or the platform reports no deck position.
    Unknown = 0,
    /// Top-row slot 0 (far left).
    TopRow0 = 1,
    /// Top-row slot 1.
    TopRow1 = 2,
    /// Top-row slot 2.
    TopRow2 = 3,
    /// Top-row slot 3.
    TopRow3 = 4,
    /// Top-row slot 4 (far right).
    TopRow4 = 5,
    /// Touchpad in the lower section.
    Touchpad = 6,
    /// The hub board all input modules connect through.
    HubBoard = 7,
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
    pub position: FrameworkInputModulePosition,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkExpansionCardModuleDescriptor {
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
    pub pd: FrameworkEcPdPortState,
    pub card_type: FrameworkExpansionCardType,
    pub card_confidence: FrameworkModuleConfidence,
    /// Static per-platform slot capability (data lane / DisplayPort / charging). Independent of `pd`.
    pub capability: FrameworkUsbCPortCapability,
    pub reserved: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkModuleInventory {
    pub usb_c_slot_count: u8,
    pub input_top_row_count: u8,
    pub detached_count: u8,
    pub reserved_0: u8,
    pub usb_c_slot_0: FrameworkExpansionCardModuleDescriptor,
    pub usb_c_slot_1: FrameworkExpansionCardModuleDescriptor,
    pub usb_c_slot_2: FrameworkExpansionCardModuleDescriptor,
    pub usb_c_slot_3: FrameworkExpansionCardModuleDescriptor,
    pub usb_c_slot_4: FrameworkExpansionCardModuleDescriptor,
    pub usb_c_slot_5: FrameworkExpansionCardModuleDescriptor,
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkPrivacySwitchesResult {
    pub status: FrameworkStatus,
    pub microphone_enabled: u8,
    pub camera_enabled: u8,
    pub reserved: [u8; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkChargeLimitsResult {
    pub status: FrameworkStatus,
    pub min_percent: u8,
    pub max_percent: u8,
    pub reserved: [u8; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkChassisIntrusionResult {
    pub status: FrameworkStatus,
    pub currently_open: u8,
    pub coin_cell_ever_removed: u8,
    pub ever_opened: u8,
    pub total_opened: u8,
    pub vtr_open_count: u8,
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcUptimeResult {
    pub status: FrameworkStatus,
    pub time_since_ec_boot_ms: u32,
    pub ap_resets_since_ec_boot: u32,
    pub ec_reset_flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkS0ixCounterResult {
    pub status: FrameworkStatus,
    pub s0ix_count: u32,
    pub reserved: u32,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkTabletModeOverride {
    Default = 0,
    ForceTablet = 1,
    ForceClamshell = 2,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkDeckStateMode {
    ReadOnly = 0,
    Required = 1,
    ForceOn = 2,
    ForceOff = 4,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkBoardIdType {
    Mainboard = 0,
    PowerButtonBoard = 1,
    Touchpad = 2,
    AudioBoard = 3,
    DGpu0 = 4,
    DGpu1 = 5,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkBoardIdResult {
    pub status: FrameworkStatus,
    pub board_id_type: FrameworkBoardIdType,
    pub board_id: i8, // -1 = invalid, 0–14 = version, 15 = not present
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkActiveChargeResult {
    pub status: FrameworkStatus,
    pub active_port_index: i8, // -1 = none
    pub reserved: [u8; 3],
    pub pd: FrameworkEcPdPortState,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkSensorCategory {
    Motion = 0,
    Environmental = 1,
    Other = 2,
    Unknown = -1,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkSensorType {
    Accel = 0,
    Gyro = 1,
    Mag = 2,
    Prox = 3,
    Light = 4,
    Activity = 5,
    Baro = 6,
    Sync = 7,
    LightRgb = 8,
    Unknown = -1,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkSensorLocation {
    Base = 0,
    Lid = 1,
    Camera = 2,
    Unknown = -1,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkSensorChip {
    Kxcj9 = 0,
    Lsm6ds0 = 1,
    Bmi160 = 2,
    Si1141 = 3,
    Si1142 = 4,
    Si1143 = 5,
    Kx022 = 6,
    L3gd20h = 7,
    Bma255 = 8,
    Bmp280 = 9,
    Opt3001 = 10,
    Bh1730 = 11,
    Gpio = 12,
    Lis2dh = 13,
    Lsm6dsm = 14,
    Lis2de = 15,
    Lis2mdl = 16,
    Lsm6ds3 = 17,
    Lsm6dso = 18,
    Lng2dm = 19,
    Tcs3400 = 20,
    Lis2dw12 = 21,
    Lis2dwl = 22,
    Lis2ds = 23,
    Bmi260 = 24,
    Icm426xx = 25,
    Icm42607 = 26,
    Bma422 = 27,
    Bmi323 = 28,
    Bmi220 = 29,
    Cm32183 = 30,
    Veml3328 = 31,
    Unknown = -1,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkSensorDescriptor {
    pub category: FrameworkSensorCategory,
    pub sensor_type: FrameworkSensorType,
    pub location: FrameworkSensorLocation,
    pub chip: FrameworkSensorChip,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkSensorInfoSnapshot {
    pub status: FrameworkStatus,
    pub sensor_count: u8,
    pub reserved: [u8; 3],
    pub sensor_0: FrameworkSensorDescriptor,
    pub sensor_1: FrameworkSensorDescriptor,
    pub sensor_2: FrameworkSensorDescriptor,
    pub sensor_3: FrameworkSensorDescriptor,
    pub sensor_4: FrameworkSensorDescriptor,
    pub sensor_5: FrameworkSensorDescriptor,
    pub sensor_6: FrameworkSensorDescriptor,
    pub sensor_7: FrameworkSensorDescriptor,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkAccelDataResult {
    pub status: FrameworkStatus,
    pub lid_angle_degrees: i16, // -1 = unreliable / accelerometers not present
    pub reserved: [u8; 2],
    pub base_x: i16,
    pub base_y: i16,
    pub base_z: i16,
    pub lid_x: i16,
    pub lid_y: i16,
    pub lid_z: i16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkAlsResult {
    pub status: FrameworkStatus,
    pub lux_0: u32,
    pub lux_1: u32,
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
pub unsafe extern "C" fn framework_ec_set_keyboard_backlight(
    handle: *const FrameworkEcHandle,
    percent: u8,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    if percent > 100 {
        return FrameworkStatus::with(FrameworkStatusCode::InvalidArgument, 0);
    }
    handle.ec.set_keyboard_backlight(percent);
    FrameworkStatus::success()
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// `level` must be a valid `FrameworkFingerprintLedLevel` variant. `Unknown` and `Custom`
/// are rejected with `InvalidArgument` — `Custom` is a get-only level per the EC protocol.
pub unsafe extern "C" fn framework_ec_set_fingerprint_led(
    handle: *const FrameworkEcHandle,
    level: FrameworkFingerprintLedLevel,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    let Some(led_level) = inventory::fp_led_brightness_level(level) else {
        return FrameworkStatus::with(FrameworkStatusCode::InvalidArgument, 0);
    };
    match handle.ec.set_fp_led_level(led_level) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_privacy_switches(
    handle: *const FrameworkEcHandle,
) -> FrameworkPrivacySwitchesResult {
    let fail = |status| FrameworkPrivacySwitchesResult {
        status,
        microphone_enabled: 0,
        camera_enabled: 0,
        reserved: [0; 2],
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match handle.ec.get_privacy_info() {
        Ok((mic, cam)) => FrameworkPrivacySwitchesResult {
            status: FrameworkStatus::success(),
            microphone_enabled: u8::from(mic),
            camera_enabled: u8::from(cam),
            reserved: [0; 2],
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_charge_limits(
    handle: *const FrameworkEcHandle,
) -> FrameworkChargeLimitsResult {
    let fail = |status| FrameworkChargeLimitsResult {
        status,
        min_percent: 0,
        max_percent: 0,
        reserved: [0; 2],
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match handle.ec.get_charge_limit() {
        Ok((min, max)) => FrameworkChargeLimitsResult {
            status: FrameworkStatus::success(),
            min_percent: min,
            max_percent: max,
            reserved: [0; 2],
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_set_charge_limits(
    handle: *const FrameworkEcHandle,
    min_percent: u8,
    max_percent: u8,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    match handle.ec.set_charge_limit(min_percent, max_percent) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_read_board_id(
    handle: *const FrameworkEcHandle,
    board_id_type: FrameworkBoardIdType,
) -> FrameworkBoardIdResult {
    let fail = |status| FrameworkBoardIdResult {
        status,
        board_id_type,
        board_id: -1,
        reserved: [0; 3],
    };
    let handle = match require_handle(handle) {
        Ok(h) => h,
        Err(s) => return fail(s),
    };
    match controls::read_board_id(&handle.ec, board_id_type as u8) {
        Ok(board_id) => FrameworkBoardIdResult {
            status: FrameworkStatus::success(),
            board_id_type,
            board_id,
            reserved: [0; 3],
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// Scans ports 0–5 and returns the first whose `active_port` flag is set.
/// Returns `active_port_index = -1` with a zeroed `pd` field if no port is
/// actively charging.
pub unsafe extern "C" fn framework_ec_get_active_charge(
    handle: *const FrameworkEcHandle,
) -> FrameworkActiveChargeResult {
    let fail = |status| FrameworkActiveChargeResult {
        status,
        active_port_index: -1,
        reserved: [0; 3],
        pd: pd::default_pd_port_state(),
    };
    let handle = match require_handle(handle) {
        Ok(h) => h,
        Err(s) => return fail(s),
    };
    for port in 0u8..6 {
        let state = pd::query_pd_port_state(&handle.ec, port);
        if state.active_port != 0 {
            return FrameworkActiveChargeResult {
                status: FrameworkStatus::success(),
                active_port_index: port as i8,
                reserved: [0; 3],
                pd: state,
            };
        }
    }
    FrameworkActiveChargeResult {
        status: FrameworkStatus::success(),
        active_port_index: -1,
        reserved: [0; 3],
        pd: pd::default_pd_port_state(),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// Call once at startup to discover what sensors are present.
/// Returns `sensor_count = 0` with success on platforms where the EC does not
/// support motionsense commands.
pub unsafe extern "C" fn framework_ec_get_sensor_info(
    handle: *const FrameworkEcHandle,
) -> FrameworkSensorInfoSnapshot {
    let unknown = FrameworkSensorDescriptor {
        category: FrameworkSensorCategory::Unknown,
        sensor_type: FrameworkSensorType::Unknown,
        location: FrameworkSensorLocation::Unknown,
        chip: FrameworkSensorChip::Unknown,
    };
    let fail = |status| FrameworkSensorInfoSnapshot {
        status,
        sensor_count: 0,
        reserved: [0; 3],
        sensor_0: unknown,
        sensor_1: unknown,
        sensor_2: unknown,
        sensor_3: unknown,
        sensor_4: unknown,
        sensor_5: unknown,
        sensor_6: unknown,
        sensor_7: unknown,
    };
    let handle = match require_handle(handle) {
        Ok(h) => h,
        Err(s) => return fail(s),
    };
    let sensors = match handle.ec.motionsense_sensor_info() {
        Ok(s) => s,
        Err(EcError::Response(EcResponseStatus::InvalidCommand)) => vec![],
        Err(error) => return fail(status_from_error(error)),
    };
    let mut descriptors = [unknown; 8];
    for (i, info) in sensors.iter().take(8).enumerate() {
        descriptors[i] = controls::into_sensor_descriptor(info);
    }
    FrameworkSensorInfoSnapshot {
        status: FrameworkStatus::success(),
        sensor_count: sensors.len().min(8) as u8,
        reserved: [0; 3],
        sensor_0: descriptors[0],
        sensor_1: descriptors[1],
        sensor_2: descriptors[2],
        sensor_3: descriptors[3],
        sensor_4: descriptors[4],
        sensor_5: descriptors[5],
        sensor_6: descriptors[6],
        sensor_7: descriptors[7],
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// Reads base and lid accelerometer x/y/z from EC memory.
/// `lid_angle_degrees` is -1 when the EC reports the angle as unreliable, or
/// when the platform has no lid accelerometer (Desktop, some FW16 configs).
pub unsafe extern "C" fn framework_ec_get_accel_data(
    handle: *const FrameworkEcHandle,
) -> FrameworkAccelDataResult {
    let fail = |status| FrameworkAccelDataResult {
        status,
        lid_angle_degrees: -1,
        reserved: [0; 2],
        base_x: 0,
        base_y: 0,
        base_z: 0,
        lid_x: 0,
        lid_y: 0,
        lid_z: 0,
    };
    let handle = match require_handle(handle) {
        Ok(h) => h,
        Err(s) => return fail(s),
    };
    controls::get_accel_data(&handle.ec).unwrap_or_else(|| {
        fail(FrameworkStatus::with(
            FrameworkStatusCode::DataUnavailable,
            0,
        ))
    })
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// Reads both ALS lux readings from EC memory (two 16-bit values at EC_MEMMAP_ALS).
/// Returns 0 lux on platforms without an ALS sensor; the EC memory is zero in that case.
pub unsafe extern "C" fn framework_ec_get_als_reading(
    handle: *const FrameworkEcHandle,
) -> FrameworkAlsResult {
    let fail = |status| FrameworkAlsResult {
        status,
        lux_0: 0,
        lux_1: 0,
    };
    let handle = match require_handle(handle) {
        Ok(h) => h,
        Err(s) => return fail(s),
    };
    match controls::get_als(&handle.ec) {
        Some((lux_0, lux_1)) => FrameworkAlsResult {
            status: FrameworkStatus::success(),
            lux_0,
            lux_1,
        },
        None => fail(FrameworkStatus::with(
            FrameworkStatusCode::DataUnavailable,
            0,
        )),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_chassis_intrusion(
    handle: *const FrameworkEcHandle,
) -> FrameworkChassisIntrusionResult {
    let fail = |status| FrameworkChassisIntrusionResult {
        status,
        currently_open: 0,
        coin_cell_ever_removed: 0,
        ever_opened: 0,
        total_opened: 0,
        vtr_open_count: 0,
        reserved: [0; 3],
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match handle.ec.get_intrusion_status() {
        Ok(s) => FrameworkChassisIntrusionResult {
            status: FrameworkStatus::success(),
            currently_open: u8::from(s.currently_open),
            coin_cell_ever_removed: u8::from(s.coin_cell_ever_removed),
            ever_opened: u8::from(s.ever_opened),
            total_opened: s.total_opened,
            vtr_open_count: s.vtr_open_count,
            reserved: [0; 3],
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// Pass `battery_soc = -1` to apply the limit unconditionally; pass 0–100 to
/// apply it only below that state-of-charge percentage.
pub unsafe extern "C" fn framework_ec_set_charge_current_limit(
    handle: *const FrameworkEcHandle,
    current_ma: u32,
    battery_soc: i32,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    let soc = if battery_soc < 0 {
        None
    } else {
        Some(battery_soc as u32)
    };
    match handle.ec.set_charge_current_limit(current_ma, soc) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_uptime(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcUptimeResult {
    let fail = |status| FrameworkEcUptimeResult {
        status,
        time_since_ec_boot_ms: 0,
        ap_resets_since_ec_boot: 0,
        ec_reset_flags: 0,
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match controls::get_uptime(&handle.ec) {
        Ok(info) => FrameworkEcUptimeResult {
            status: FrameworkStatus::success(),
            time_since_ec_boot_ms: info.time_since_ec_boot_ms,
            ap_resets_since_ec_boot: info.ap_resets_since_ec_boot,
            ec_reset_flags: info.ec_reset_flags,
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_s0ix_counter(
    handle: *const FrameworkEcHandle,
) -> FrameworkS0ixCounterResult {
    let fail = |status| FrameworkS0ixCounterResult {
        status,
        s0ix_count: 0,
        reserved: 0,
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match handle.ec.get_s0ix_counter() {
        Ok(count) => FrameworkS0ixCounterResult {
            status: FrameworkStatus::success(),
            s0ix_count: count,
            reserved: 0,
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_reset_s0ix_counter(
    handle: *const FrameworkEcHandle,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    match handle.ec.reset_s0ix_counter() {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// Returns `EcResponse(InvalidCommand)` on platforms without a tablet hinge sensor
/// (Framework 16, Desktop).
pub unsafe extern "C" fn framework_ec_set_tablet_mode(
    handle: *const FrameworkEcHandle,
    mode: FrameworkTabletModeOverride,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    match controls::set_tablet_mode(&handle.ec, mode) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// Only meaningful on Framework 16. On other platforms the EC returns
/// `EcResponse(InvalidCommand)`.
pub unsafe extern "C" fn framework_ec_set_input_deck_mode(
    handle: *const FrameworkEcHandle,
    mode: FrameworkDeckStateMode,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    match handle
        .ec
        .set_input_deck_mode(controls::into_deck_state_mode(mode))
    {
        Ok(_) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
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
pub unsafe extern "C" fn framework_ec_get_gpu_descriptor_header(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcGpuDescriptorHeaderResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            let mut result = gpu_descriptor::default_header_result();
            result.status = status;
            return result;
        }
    };

    match gpu_descriptor::read_header(handle) {
        Ok(header) => gpu_descriptor_header_result(FrameworkStatus::success(), header),
        Err(status) => gpu_descriptor_header_result(status, gpu_descriptor::default_header()),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library. The returned
/// `descriptor` buffer must be released with `framework_byte_buffer_free`.
pub unsafe extern "C" fn framework_ec_read_gpu_descriptor(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcGpuDescriptorReadResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            let mut result = gpu_descriptor::default_read_result();
            result.status = status;
            return result;
        }
    };

    match gpu_descriptor::read_raw_descriptor(handle) {
        Ok(descriptor) => gpu_descriptor_read_result(FrameworkStatus::success(), descriptor),
        Err(status) => gpu_descriptor_read_result(status, FrameworkByteBuffer::default()),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
/// `expected_descriptor_ptr` must be valid for `expected_descriptor_length`
/// bytes when `expected_descriptor_length` is greater than 0.
pub unsafe extern "C" fn framework_ec_validate_gpu_descriptor(
    handle: *const FrameworkEcHandle,
    expected_descriptor_ptr: *const u8,
    expected_descriptor_length: u32,
) -> FrameworkEcGpuDescriptorValidationResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            let mut result = gpu_descriptor::default_validation_result();
            result.status = status;
            return result;
        }
    };

    let expected_descriptor = match gpu_descriptor::validate_expected_bytes(
        expected_descriptor_ptr,
        expected_descriptor_length,
    ) {
        Ok(expected_descriptor) => expected_descriptor,
        Err(status) => {
            let mut result = gpu_descriptor::default_validation_result();
            result.status = status;
            return result;
        }
    };

    match gpu_descriptor::validate(handle, expected_descriptor) {
        Ok(is_match) => gpu_descriptor_validation_result(FrameworkStatus::success(), is_match),
        Err(status) => gpu_descriptor_validation_result(status, false),
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

// ---------------------------------------------------------------------------
// EC switches
// ---------------------------------------------------------------------------

/// Live EC switch positions read from the memmap switch byte.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcSwitchesResult {
    pub status: FrameworkStatus,
    /// The raw switch byte, for callers that want bits the ABI does not name.
    pub raw: u8,
    pub lid_open: u8,
    pub power_button_pressed: u8,
    /// Note the inverted sense: 1 means write protect is *disabled*.
    pub write_protect_disabled: u8,
    pub dedicated_recovery: u8,
    pub reserved: [u8; 3],
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_switches(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcSwitchesResult {
    let fail = |status| FrameworkEcSwitchesResult {
        status,
        raw: 0,
        lid_open: 0,
        power_button_pressed: 0,
        write_protect_disabled: 0,
        dedicated_recovery: 0,
        reserved: [0; 3],
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    let Some(switches) = diagnostics::get_switches(&handle.ec) else {
        return fail(FrameworkStatus::with(
            FrameworkStatusCode::DataUnavailable,
            0,
        ));
    };

    FrameworkEcSwitchesResult {
        status: FrameworkStatus::success(),
        raw: switches.raw,
        lid_open: u8::from(switches.lid_open),
        power_button_pressed: u8::from(switches.power_button_pressed),
        write_protect_disabled: u8::from(switches.write_protect_disabled),
        dedicated_recovery: u8::from(switches.dedicated_recovery),
        reserved: [0; 3],
    }
}

// ---------------------------------------------------------------------------
// EC liveness and protocol
// ---------------------------------------------------------------------------

/// Result of the EC `hello` diagnostic command.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcHelloResult {
    pub status: FrameworkStatus,
    /// What the EC echoed back: `in_data + 0x01020304` on a healthy EC.
    pub out_data: u32,
    /// 1 when `out_data` matched the expected echo, 0 otherwise.
    pub is_expected: u8,
    pub reserved: [u8; 3],
}

#[no_mangle]
/// Sends the EC `hello` command and checks the echo.
///
/// Pass any `in_data`; the EC must return `in_data + 0x01020304`. This is the
/// cheapest end-to-end check that EC host communication actually works.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_hello(
    handle: *const FrameworkEcHandle,
    in_data: u32,
) -> FrameworkEcHelloResult {
    let fail = |status| FrameworkEcHelloResult {
        status,
        out_data: 0,
        is_expected: 0,
        reserved: [0; 3],
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match handle.ec.hello(in_data) {
        Ok(out_data) => FrameworkEcHelloResult {
            status: FrameworkStatus::success(),
            out_data,
            is_expected: u8::from(out_data == in_data.wrapping_add(diagnostics::HELLO_OFFSET)),
            reserved: [0; 3],
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// Convenience wrapper over `framework_ec_hello` using the same magic value
/// upstream's `check_hello` uses.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_check_hello(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcHelloResult {
    framework_ec_hello(handle, diagnostics::HELLO_DEFAULT_IN_DATA)
}

/// Set when `EC_RES_IN_PROGRESS` may be returned by a slow command.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcProtocolFlag {
    InProgressSupported = 0x01,
}

/// EC host command protocol capabilities.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcProtocolInfoResult {
    pub status: FrameworkStatus,
    /// Bitmask of supported protocol versions; bit N means version N.
    pub protocol_versions: u32,
    pub max_request_packet_size: u16,
    pub max_response_packet_size: u16,
    /// See `FrameworkEcProtocolFlag`.
    pub flags: u32,
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_protocol_info(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcProtocolInfoResult {
    let fail = |status| FrameworkEcProtocolInfoResult {
        status,
        protocol_versions: 0,
        max_request_packet_size: 0,
        max_response_packet_size: 0,
        flags: 0,
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match diagnostics::get_protocol_info(&handle.ec) {
        Ok(info) => FrameworkEcProtocolInfoResult {
            status: FrameworkStatus::success(),
            protocol_versions: info.protocol_versions,
            max_request_packet_size: info.max_request_packet_size,
            max_response_packet_size: info.max_response_packet_size,
            flags: info.flags,
        },
        Err(error) => fail(status_from_error(error)),
    }
}

// ---------------------------------------------------------------------------
// Sysinfo
// ---------------------------------------------------------------------------

/// Bit values of `FrameworkEcSysinfoResult::flags`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcSysinfoFlag {
    /// Write protect is asserted, debug features are disabled.
    Locked = 0x01,
    /// Locked even if write protect is deasserted.
    ForceLocked = 0x02,
    /// Jumping between images is enabled.
    JumpEnabled = 0x04,
    /// EC jumped directly to the current image at boot.
    JumpedToCurrentImage = 0x08,
    /// EC will reboot when the system shuts down.
    RebootAtShutdown = 0x10,
    /// System is in manual recovery mode.
    InManualRecovery = 0x20,
}

/// Bit values of the EC reset-flag words reported by sysinfo and uptime.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcResetFlag {
    Other = 0x0000_0001,
    ResetPin = 0x0000_0002,
    Brownout = 0x0000_0004,
    PowerOn = 0x0000_0008,
    Watchdog = 0x0000_0010,
    Soft = 0x0000_0020,
    Hibernate = 0x0000_0040,
    RtcAlarm = 0x0000_0080,
    WakePin = 0x0000_0100,
    LowBattery = 0x0000_0200,
    Sysjump = 0x0000_0400,
    Hard = 0x0000_0800,
    ApOff = 0x0000_1000,
    Preserved = 0x0000_2000,
    UsbResume = 0x0000_4000,
    Rdd = 0x0000_8000,
    Rbox = 0x0001_0000,
    Security = 0x0002_0000,
    ApWatchdog = 0x0004_0000,
    StayInRo = 0x0008_0000,
    Efs = 0x0010_0000,
    ApIdle = 0x0020_0000,
    InitialPwr = 0x0040_0000,
}

/// EC system information: which image is running and why it booted.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcSysinfoResult {
    pub status: FrameworkStatus,
    pub current_image: FrameworkEcCurrentImage,
    /// See `FrameworkEcResetFlag`.
    pub reset_flags: u32,
    /// See `FrameworkEcSysinfoFlag`.
    pub flags: u32,
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_sysinfo(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcSysinfoResult {
    let fail = |status| FrameworkEcSysinfoResult {
        status,
        current_image: FrameworkEcCurrentImage::Unknown,
        reset_flags: 0,
        flags: 0,
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match diagnostics::get_sysinfo(&handle.ec) {
        Ok(info) => FrameworkEcSysinfoResult {
            status: FrameworkStatus::success(),
            current_image: match info.current_image {
                1 => FrameworkEcCurrentImage::Ro,
                2 => FrameworkEcCurrentImage::Rw,
                _ => FrameworkEcCurrentImage::Unknown,
            },
            reset_flags: info.reset_flags,
            flags: info.flags,
        },
        Err(error) => fail(status_from_error(error)),
    }
}

// ---------------------------------------------------------------------------
// Panic info
// ---------------------------------------------------------------------------

/// Saved EC panic data.
///
/// The `data` buffer holds the raw `struct panic_data` blob and must be
/// released with `framework_byte_buffer_free`. A zero-length buffer with a
/// success status means the EC simply has no stored panic.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcPanicInfoResult {
    pub status: FrameworkStatus,
    pub data: FrameworkByteBuffer,
    /// Architecture tag from the blob header.
    pub arch: u8,
    pub struct_version: u8,
    /// Panic data flags from the blob header.
    pub flags: u8,
    /// 1 when the trailer magic and struct size both agree with the payload.
    pub is_valid: u8,
    /// Struct size the EC reported in the blob trailer.
    pub struct_size: u32,
    /// Trailer magic; `0x21636E50` ("Pnc!") when valid.
    pub magic: u32,
}

#[no_mangle]
/// Reads stored EC panic data without clearing the EC's "already read" flag
/// where the EC supports the v1 command.
///
/// The returned `data` buffer must be released with `framework_byte_buffer_free`.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_panic_info(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcPanicInfoResult {
    let fail = |status| FrameworkEcPanicInfoResult {
        status,
        data: FrameworkByteBuffer::default(),
        arch: 0,
        struct_version: 0,
        flags: 0,
        is_valid: 0,
        struct_size: 0,
        magic: 0,
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match diagnostics::get_panic_info(&handle.ec) {
        Ok(info) => FrameworkEcPanicInfoResult {
            status: FrameworkStatus::success(),
            data: FrameworkByteBuffer::from_vec(info.data),
            arch: info.arch,
            struct_version: info.struct_version,
            flags: info.flags,
            is_valid: u8::from(info.is_valid),
            struct_size: info.struct_size,
            magic: info.magic,
        },
        Err(error) => fail(status_from_error(error)),
    }
}

// ---------------------------------------------------------------------------
// Port 80 history
// ---------------------------------------------------------------------------

/// Marker codes the EC inserts into the port 80 history.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPort80Event {
    /// The system resumed (S3 -> S0).
    Resume = 0x1001,
    /// The system reset.
    Reset = 0x1002,
}

/// Port 80 POST code history read from the EC.
///
/// `codes` holds `history_size` little-endian `u16` entries in buffer order, so
/// its length is `history_size * 2` bytes. The newest entry sits at
/// `writes % history_size`. Release it with `framework_byte_buffer_free`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcPort80HistoryResult {
    pub status: FrameworkStatus,
    /// Total number of port 80 writes so far.
    pub writes: u32,
    /// Size of the history buffer in entries.
    pub history_size: u32,
    pub codes: FrameworkByteBuffer,
}

#[no_mangle]
/// The returned `codes` buffer must be released with `framework_byte_buffer_free`.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_port80_history(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcPort80HistoryResult {
    let fail = |status| FrameworkEcPort80HistoryResult {
        status,
        writes: 0,
        history_size: 0,
        codes: FrameworkByteBuffer::default(),
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match diagnostics::port80_read(&handle.ec) {
        Ok(history) => FrameworkEcPort80HistoryResult {
            status: FrameworkStatus::success(),
            writes: history.writes,
            history_size: history.history_size,
            codes: FrameworkByteBuffer::from_vec(history.codes),
        },
        Err(error) => fail(status_from_error(error)),
    }
}

// ---------------------------------------------------------------------------
// Battery cutoff and AP throttle
// ---------------------------------------------------------------------------

/// Battery cutoff (ship mode) state.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkBatteryCutoffState {
    /// The EC did not answer the cutoff query.
    Unknown = 0,
    NotCutOff = 1,
    CutOff = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcBatteryCutoffResult {
    pub status: FrameworkStatus,
    pub state: FrameworkBatteryCutoffState,
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_battery_cutoff_status(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcBatteryCutoffResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => {
            return FrameworkEcBatteryCutoffResult {
                status,
                state: FrameworkBatteryCutoffState::Unknown,
            }
        }
    };
    match thermal::get_cutoff_status(&handle.ec) {
        Some(cut_off) => FrameworkEcBatteryCutoffResult {
            status: FrameworkStatus::success(),
            state: if cut_off {
                FrameworkBatteryCutoffState::CutOff
            } else {
                FrameworkBatteryCutoffState::NotCutOff
            },
        },
        None => FrameworkEcBatteryCutoffResult {
            status: FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            state: FrameworkBatteryCutoffState::Unknown,
        },
    }
}

/// Whether the EC is currently throttling the AP for thermal reasons.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcApThrottleResult {
    pub status: FrameworkStatus,
    pub soft_throttled: u8,
    pub hard_throttled: u8,
    pub reserved: [u8; 2],
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_ap_throttle_status(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcApThrottleResult {
    let fail = |status| FrameworkEcApThrottleResult {
        status,
        soft_throttled: 0,
        hard_throttled: 0,
        reserved: [0; 2],
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match thermal::get_ap_throttle(&handle.ec) {
        Ok(throttle) => FrameworkEcApThrottleResult {
            status: FrameworkStatus::success(),
            soft_throttled: u8::from(throttle.soft),
            hard_throttled: u8::from(throttle.hard),
            reserved: [0; 2],
        },
        Err(error) => fail(status_from_error(error)),
    }
}

// ---------------------------------------------------------------------------
// Thermal thresholds
// ---------------------------------------------------------------------------

/// Bit values of `FrameworkThermalThresholds::enabled_mask`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkThermalThresholdFlag {
    Warn = 0x01,
    High = 0x02,
    Halt = 0x04,
    WarnRelease = 0x08,
    HighRelease = 0x10,
    HaltRelease = 0x20,
    FanOff = 0x40,
    FanMax = 0x80,
}

/// EC thermal configuration for one temperature sensor.
///
/// The EC stores thresholds in Kelvin with zero meaning "disabled". This ABI
/// reports degrees Celsius, so a disabled threshold reads back as -273; always
/// consult `enabled_mask` rather than testing the Celsius value.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkThermalThresholds {
    /// See `FrameworkThermalThresholdFlag`. A clear bit means that threshold is
    /// disabled in EC firmware.
    pub enabled_mask: u32,
    /// AP is warned above this temperature.
    pub warn_celsius: i32,
    /// EC throttles the AP above this temperature.
    pub high_celsius: i32,
    /// EC shuts the system down above this temperature.
    pub halt_celsius: i32,
    /// Release point for `warn_celsius`; 0 K in firmware means default 1 degree hysteresis.
    pub warn_release_celsius: i32,
    /// Release point for `high_celsius`.
    pub high_release_celsius: i32,
    /// Release point for `halt_celsius`.
    pub halt_release_celsius: i32,
    /// No active cooling is needed below this temperature.
    pub fan_off_celsius: i32,
    /// Maximum active cooling is applied above this temperature.
    pub fan_max_celsius: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcThermalThresholdsResult {
    pub status: FrameworkStatus,
    pub sensor_index: u32,
    pub thresholds: FrameworkThermalThresholds,
}

#[no_mangle]
/// Reads the EC thermal configuration for one temperature sensor.
///
/// `sensor_index` matches the temperature slot index used by
/// `framework_ec_get_thermal_snapshot`.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_thermal_thresholds(
    handle: *const FrameworkEcHandle,
    sensor_index: u32,
) -> FrameworkEcThermalThresholdsResult {
    let fail = |status| FrameworkEcThermalThresholdsResult {
        status,
        sensor_index,
        thresholds: FrameworkThermalThresholds {
            enabled_mask: 0,
            warn_celsius: 0,
            high_celsius: 0,
            halt_celsius: 0,
            warn_release_celsius: 0,
            high_release_celsius: 0,
            halt_release_celsius: 0,
            fan_off_celsius: 0,
            fan_max_celsius: 0,
        },
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match thermal::get_thermal_thresholds(&handle.ec, sensor_index) {
        Ok(thresholds) => FrameworkEcThermalThresholdsResult {
            status: FrameworkStatus::success(),
            sensor_index,
            thresholds,
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// Writes EC thermal thresholds for one temperature sensor.
///
/// Each value follows upstream's convention: a negative value keeps the current
/// threshold, zero disables it, and a positive value is degrees Celsius. The
/// call is a read-modify-write, so untouched thresholds and the release points
/// are preserved.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_set_thermal_thresholds(
    handle: *const FrameworkEcHandle,
    sensor_index: u32,
    warn_celsius: i32,
    high_celsius: i32,
    halt_celsius: i32,
    fan_off_celsius: i32,
    fan_max_celsius: i32,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    let values = [
        warn_celsius,
        high_celsius,
        halt_celsius,
        fan_off_celsius,
        fan_max_celsius,
    ];
    match thermal::set_thermal_thresholds(&handle.ec, sensor_index, &values) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}

/// A temperature sensor's name as reported by EC firmware.
///
/// The `name` buffer must be released with `framework_byte_buffer_free`.
/// `mapped_name` is the same name resolved onto the stable
/// `FrameworkSensorName` enum, or `Generic` when firmware uses a name this
/// version does not recognise.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcTempSensorNameResult {
    pub status: FrameworkStatus,
    pub sensor_index: u32,
    pub mapped_name: FrameworkSensorName,
    /// EC sensor type tag: 0 ignored, 1 CPU, 2 board, 3 case, 4 battery.
    pub sensor_type: u8,
    pub reserved: [u8; 1],
    pub name: FrameworkByteBuffer,
}

#[no_mangle]
/// Reads a temperature sensor's name from EC firmware.
///
/// This costs one host command per call, so it is deliberately kept out of the
/// polled thermal snapshot: read the names once and cache them, then poll
/// `framework_ec_get_thermal_snapshot` for values.
///
/// The returned `name` buffer must be released with `framework_byte_buffer_free`.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_temp_sensor_name(
    handle: *const FrameworkEcHandle,
    sensor_index: u8,
) -> FrameworkEcTempSensorNameResult {
    let fail = |status| FrameworkEcTempSensorNameResult {
        status,
        sensor_index: u32::from(sensor_index),
        mapped_name: FrameworkSensorName::Unknown,
        sensor_type: 0,
        reserved: [0; 1],
        name: FrameworkByteBuffer::default(),
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    match thermal::get_temp_sensor_info(&handle.ec, sensor_index) {
        Ok(info) => FrameworkEcTempSensorNameResult {
            status: FrameworkStatus::success(),
            sensor_index: u32::from(sensor_index),
            mapped_name: thermal::map_sensor_name(&info.name),
            sensor_type: info.sensor_type,
            reserved: [0; 1],
            name: FrameworkByteBuffer::from_vec(info.name.into_bytes()),
        },
        Err(error) => fail(status_from_error(error)),
    }
}

/// Result of the Smart Battery SHA-1 HMAC authentication challenge.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcBatteryAuthResult {
    pub status: FrameworkStatus,
    /// 1 when the battery's response matched the expected challenge. A zero with a
    /// success status means the battery answered but failed authentication.
    pub authenticated: u8,
    pub reserved: [u8; 3],
}

#[no_mangle]
/// Runs the Smart Battery SHA-1 HMAC challenge/response authentication.
///
/// `auth_key` must point to exactly 16 bytes. Passing null, or a battery that does
/// not answer, yields a non-success status; a battery that answers but fails the
/// challenge yields success with `authenticated = 0`.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library, and `auth_key` must
/// point to at least 16 readable bytes.
pub unsafe extern "C" fn framework_ec_authenticate_battery(
    handle: *const FrameworkEcHandle,
    auth_key: *const u8,
) -> FrameworkEcBatteryAuthResult {
    let fail = |status| FrameworkEcBatteryAuthResult {
        status,
        authenticated: 0,
        reserved: [0; 3],
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    if auth_key.is_null() {
        return fail(FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0));
    }

    let mut key = [0u8; 16];
    key.copy_from_slice(std::slice::from_raw_parts(auth_key, 16));

    match smart_battery::authenticate(&handle.ec, &key) {
        Ok(authenticated) => FrameworkEcBatteryAuthResult {
            status: FrameworkStatus::success(),
            authenticated: u8::from(authenticated),
            reserved: [0; 3],
        },
        Err(error) => fail(status_from_error(error)),
    }
}

// ---------------------------------------------------------------------------
// Smart Battery (SBS)
// ---------------------------------------------------------------------------

/// Full Smart Battery data set read over I2C passthrough.
///
/// All five text/blob buffers are owned by the caller. Release the whole record
/// with `framework_smart_battery_data_free`; do not free the individual buffers.
///
/// Fields in the unsealed group (`state_of_health`, `operation_status`,
/// `safety_alert`, `safety_status`, `pf_alert`, `pf_status`, `lifetime_1`
/// through `lifetime_5`) are only populated when an unseal key was supplied and
/// accepted; check `unsealed`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkSmartBatteryData {
    /// SBS BatteryMode register.
    pub mode: u16,
    pub serial_number: u16,
    /// Raw packed SBS ManufactureDate word.
    pub manufacture_date_raw: u16,
    pub manufacture_year: u16,
    pub manufacture_month: u8,
    pub manufacture_day: u8,
    /// Tenths of a Kelvin: degrees Celsius = value / 10 - 273.15.
    pub temperature_decikelvin: u16,
    pub voltage_mv: u16,
    pub cell_voltage_1_mv: u16,
    pub cell_voltage_2_mv: u16,
    pub cell_voltage_3_mv: u16,
    pub cell_voltage_4_mv: u16,
    pub cycle_count: u16,
    /// Signed: negative while discharging.
    pub current_ma: i16,
    pub avg_current_ma: i16,
    pub rel_state_of_charge: u16,
    pub abs_state_of_charge: u16,
    pub remaining_capacity: u16,
    pub full_charge_capacity: u16,
    pub charging_current_ma: i16,
    pub charging_voltage_mv: u16,
    /// SBS BatteryStatus register.
    pub battery_status: u16,
    pub design_capacity: u16,
    pub design_voltage_mv: u16,
    pub operation_status: u32,
    pub safety_alert: u32,
    pub safety_status: u32,
    pub pf_alert: u32,
    pub pf_status: u32,
    /// 1 when the unsealed register group was read successfully.
    pub unsealed: u8,
    pub reserved: [u8; 3],
    pub device_name: FrameworkByteBuffer,
    pub manufacturer_name: FrameworkByteBuffer,
    pub device_chemistry: FrameworkByteBuffer,
    /// Raw state-of-health block; first two LE u16s are mAh and cWh.
    pub state_of_health: FrameworkByteBuffer,
    /// Raw MAC 0x0002 response: subcmd echo, device number, firmware version, build.
    pub firmware_version: FrameworkByteBuffer,
    pub lifetime_1: FrameworkByteBuffer,
    pub lifetime_2: FrameworkByteBuffer,
    pub lifetime_3: FrameworkByteBuffer,
    pub lifetime_4: FrameworkByteBuffer,
    pub lifetime_5: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcSmartBatteryResult {
    pub status: FrameworkStatus,
    pub data: FrameworkSmartBatteryData,
}

#[no_mangle]
/// Reads the full Smart Battery data set.
///
/// This performs many I2C passthrough round trips and is far slower than
/// `framework_ec_get_power_snapshot`; call it on demand, not in a poll loop.
///
/// Set `use_unseal_key` to a non-zero value to unlock the manufacturer-access
/// registers using `unseal_key`; pass zero to read only the sealed-mode subset.
///
/// The returned record must be released with `framework_smart_battery_data_free`.
///
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_smart_battery_data(
    handle: *const FrameworkEcHandle,
    use_unseal_key: u8,
    unseal_key: u32,
) -> FrameworkEcSmartBatteryResult {
    let fail = |status| FrameworkEcSmartBatteryResult {
        status,
        data: smart_battery::default_ffi(),
    };
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fail(status),
    };
    let key = if use_unseal_key != 0 {
        Some(unseal_key)
    } else {
        None
    };
    match smart_battery::collect(&handle.ec, key) {
        Ok(data) => FrameworkEcSmartBatteryResult {
            status: FrameworkStatus::success(),
            data: smart_battery::into_ffi(data),
        },
        Err(error) => fail(status_from_error(error)),
    }
}

#[no_mangle]
/// Releases every buffer owned by a `FrameworkSmartBatteryData`.
///
/// # Safety
/// `data` must point to a record returned by
/// `framework_ec_get_smart_battery_data` that has not already been freed.
/// Passing null is a no-op.
pub unsafe extern "C" fn framework_smart_battery_data_free(data: *mut FrameworkSmartBatteryData) {
    if data.is_null() {
        return;
    }

    let data = &mut *data;
    for buffer in [
        &mut data.device_name,
        &mut data.manufacturer_name,
        &mut data.device_chemistry,
        &mut data.state_of_health,
        &mut data.firmware_version,
        &mut data.lifetime_1,
        &mut data.lifetime_2,
        &mut data.lifetime_3,
        &mut data.lifetime_4,
        &mut data.lifetime_5,
    ] {
        let owned = *buffer;
        *buffer = FrameworkByteBuffer::default();
        owned.destroy();
    }
}
