use std::collections::VecDeque;
use std::convert::TryFrom;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use framework_lib::audio_card;
use framework_lib::camera;
use framework_lib::ccgx::hid;
use framework_lib::chromium_ec::commands::{
    BoardIdType, EcFeatureCode, EcRequestExpansionBayStatus, EcRequestGetFeatures,
    EcRequestGetGpuPcie, EcRequestGetPdPortState, ExpansionBayBoard, ExpansionBayIssue,
    FpLedBrightnessLevel, GpuPcieConfig, GpuVendor,
};
use framework_lib::chromium_ec::input_deck::InputModuleType;
use framework_lib::chromium_ec::{
    CrosEc, CrosEcDriver, CrosEcDriverType, EcCurrentImage, EcError, EcRequestRaw, EcResponseStatus,
};
use framework_lib::inputmodule;
use framework_lib::power;
use framework_lib::smbios;
use framework_lib::smbios::{Platform, PlatformFamily};
use framework_lib::touchpad;
use framework_lib::touchscreen;
use hidapi::HidApi;

const STORED_DEVICE_ERROR_LIMIT: usize = 64;
#[cfg(target_os = "linux")]
const CROS_EC_DEV_PATH: &str = "/dev/cros_ec";
const MAX_USB_C_SLOT_COUNT: usize = 6;
const THERMAL_SENSOR_COUNT: usize = 8;
const FAN_SLOT_COUNT: usize = 4;
const EC_MEMMAP_TEMP_SENSOR: u16 = 0x00;
const EC_MEMMAP_FAN: u16 = 0x10;
const EC_FAN_SPEED_STALLED_DEPRECATED: u16 = 0xFFFE;
const EC_FAN_SPEED_NOT_PRESENT: u16 = 0xFFFF;

static NEXT_DEVICE_ERROR_ID: AtomicI32 = AtomicI32::new(1);
static DEVICE_ERROR_MESSAGES: OnceLock<Mutex<VecDeque<(i32, String)>>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThermalSensorStatus {
    Ok,
    NotPresent,
    Error,
    NotPowered,
    NotCalibrated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ThermalSensorReading {
    status: ThermalSensorStatus,
    celsius: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ThermalSnapshot {
    temperatures: [ThermalSensorReading; THERMAL_SENSOR_COUNT],
    fan_rpms: [u16; FAN_SLOT_COUNT],
    fan_present: [bool; FAN_SLOT_COUNT],
    fan_stalled: [bool; FAN_SLOT_COUNT],
    fan_count: u8,
}

fn parse_temp_sensor(byte: u8) -> ThermalSensorReading {
    match byte {
        0xFF => ThermalSensorReading {
            status: ThermalSensorStatus::NotPresent,
            celsius: 0,
        },
        0xFE => ThermalSensorReading {
            status: ThermalSensorStatus::Error,
            celsius: 0,
        },
        0xFD => ThermalSensorReading {
            status: ThermalSensorStatus::NotPowered,
            celsius: 0,
        },
        0xFC => ThermalSensorReading {
            status: ThermalSensorStatus::NotCalibrated,
            celsius: 0,
        },
        value => ThermalSensorReading {
            status: ThermalSensorStatus::Ok,
            celsius: i16::from(value) - 73,
        },
    }
}

fn thermal_snapshot(ec: &CrosEc) -> Option<ThermalSnapshot> {
    let temps = ec.read_memory(EC_MEMMAP_TEMP_SENSOR, 0x0F)?;
    let fans = ec.read_memory(EC_MEMMAP_FAN, 0x08)?;

    let mut temperatures = [ThermalSensorReading {
        status: ThermalSensorStatus::NotPresent,
        celsius: 0,
    }; THERMAL_SENSOR_COUNT];
    for (index, byte) in temps.iter().take(THERMAL_SENSOR_COUNT).enumerate() {
        temperatures[index] = parse_temp_sensor(*byte);
    }

    let mut fan_rpms = [0u16; FAN_SLOT_COUNT];
    let mut fan_present = [false; FAN_SLOT_COUNT];
    let mut fan_stalled = [false; FAN_SLOT_COUNT];
    let mut fan_count = 0u8;

    for index in 0..FAN_SLOT_COUNT {
        let fan = u16::from_le_bytes([fans[index * 2], fans[1 + index * 2]]);
        match fan {
            EC_FAN_SPEED_NOT_PRESENT => {}
            EC_FAN_SPEED_STALLED_DEPRECATED => {
                fan_present[index] = true;
                fan_stalled[index] = true;
                fan_count += 1;
            }
            rpm => {
                fan_rpms[index] = rpm;
                fan_present[index] = true;
                fan_count += 1;
            }
        }
    }

    Some(ThermalSnapshot {
        temperatures,
        fan_rpms,
        fan_present,
        fan_stalled,
        fan_count,
    })
}

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

impl FrameworkStatus {
    fn success() -> Self {
        Self::no_payload(FrameworkStatusCode::Success)
    }

    fn with(code: FrameworkStatusCode, detail: i32) -> Self {
        match code {
            FrameworkStatusCode::Success => Self::success(),
            FrameworkStatusCode::NullPointer => Self::no_payload(FrameworkStatusCode::NullPointer),
            FrameworkStatusCode::InvalidArgument => Self::invalid_fan_index(detail),
            FrameworkStatusCode::NoDriverAvailable => {
                Self::no_payload(FrameworkStatusCode::NoDriverAvailable)
            }
            FrameworkStatusCode::UnsupportedDriver => {
                Self::no_payload(FrameworkStatusCode::UnsupportedDriver)
            }
            FrameworkStatusCode::DeviceError => Self::device_error(detail),
            FrameworkStatusCode::EcResponse => {
                Self::ec_response(ec_response_detail_from_raw(detail))
            }
            FrameworkStatusCode::UnknownResponseCode => Self::unknown_response_code(detail),
            FrameworkStatusCode::DataUnavailable => {
                Self::no_payload(FrameworkStatusCode::DataUnavailable)
            }
        }
    }

    fn no_payload(code: FrameworkStatusCode) -> Self {
        Self {
            code,
            payload: FrameworkStatusPayload {
                none: FrameworkStatusNoPayload { reserved: 0 },
            },
        }
    }

    fn invalid_fan_index(fan_index: i32) -> Self {
        Self {
            code: FrameworkStatusCode::InvalidArgument,
            payload: FrameworkStatusPayload {
                invalid_fan_index: FrameworkStatusInvalidFanIndexRecord { fan_index },
            },
        }
    }

    fn ec_response(response: FrameworkEcResponseDetail) -> Self {
        Self {
            code: FrameworkStatusCode::EcResponse,
            payload: FrameworkStatusPayload {
                ec_response: FrameworkStatusEcResponseRecord { response },
            },
        }
    }

    fn unknown_response_code(response_code: i32) -> Self {
        Self {
            code: FrameworkStatusCode::UnknownResponseCode,
            payload: FrameworkStatusPayload {
                unknown_ec_response_code: FrameworkStatusUnknownEcResponseCodeRecord {
                    response_code,
                },
            },
        }
    }

    fn device_error(message_token: i32) -> Self {
        Self {
            code: FrameworkStatusCode::DeviceError,
            payload: FrameworkStatusPayload {
                device_error: FrameworkStatusDeviceErrorRecord { message_token },
            },
        }
    }

    fn invalid_fan_index_value(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::InvalidArgument {
            return None;
        }

        Some(unsafe { self.payload.invalid_fan_index.fan_index })
    }

    fn ec_response_detail(&self) -> Option<FrameworkEcResponseDetail> {
        if self.code != FrameworkStatusCode::EcResponse {
            return None;
        }

        Some(unsafe { self.payload.ec_response.response })
    }

    fn unknown_response_code_value(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::UnknownResponseCode {
            return None;
        }

        Some(unsafe { self.payload.unknown_ec_response_code.response_code })
    }

    fn device_error_message_token(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::DeviceError {
            return None;
        }

        Some(unsafe { self.payload.device_error.message_token })
    }
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

impl From<EcResponseStatus> for FrameworkEcResponseDetail {
    fn from(value: EcResponseStatus) -> Self {
        match value {
            EcResponseStatus::Success => FrameworkEcResponseDetail::Success,
            EcResponseStatus::InvalidCommand => FrameworkEcResponseDetail::InvalidCommand,
            EcResponseStatus::Error => FrameworkEcResponseDetail::Error,
            EcResponseStatus::InvalidParameter => FrameworkEcResponseDetail::InvalidParameter,
            EcResponseStatus::AccessDenied => FrameworkEcResponseDetail::AccessDenied,
            EcResponseStatus::InvalidResponse => FrameworkEcResponseDetail::InvalidResponse,
            EcResponseStatus::InvalidVersion => FrameworkEcResponseDetail::InvalidVersion,
            EcResponseStatus::InvalidChecksum => FrameworkEcResponseDetail::InvalidChecksum,
            EcResponseStatus::InProgress => FrameworkEcResponseDetail::InProgress,
            EcResponseStatus::Unavailable => FrameworkEcResponseDetail::Unavailable,
            EcResponseStatus::Timeout => FrameworkEcResponseDetail::Timeout,
            EcResponseStatus::Overflow => FrameworkEcResponseDetail::Overflow,
            EcResponseStatus::InvalidHeader => FrameworkEcResponseDetail::InvalidHeader,
            EcResponseStatus::RequestTruncated => FrameworkEcResponseDetail::RequestTruncated,
            EcResponseStatus::ResponseTooBig => FrameworkEcResponseDetail::ResponseTooBig,
            EcResponseStatus::BusError => FrameworkEcResponseDetail::BusError,
            EcResponseStatus::Busy => FrameworkEcResponseDetail::Busy,
        }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcDriver {
    Unknown = -1,
    Portio = 0,
    CrosEc = 1,
    Windows = 2,
}

impl From<CrosEcDriverType> for FrameworkEcDriver {
    fn from(value: CrosEcDriverType) -> Self {
        match value {
            CrosEcDriverType::Portio => FrameworkEcDriver::Portio,
            CrosEcDriverType::CrosEc => FrameworkEcDriver::CrosEc,
            CrosEcDriverType::Windows => FrameworkEcDriver::Windows,
        }
    }
}

impl TryFrom<FrameworkEcDriver> for CrosEcDriverType {
    type Error = ();

    fn try_from(value: FrameworkEcDriver) -> Result<Self, Self::Error> {
        match value {
            FrameworkEcDriver::Unknown => Err(()),
            FrameworkEcDriver::Portio => Ok(CrosEcDriverType::Portio),
            FrameworkEcDriver::CrosEc => Ok(CrosEcDriverType::CrosEc),
            FrameworkEcDriver::Windows => Ok(CrosEcDriverType::Windows),
        }
    }
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

impl From<Platform> for FrameworkPlatform {
    fn from(value: Platform) -> Self {
        match value {
            Platform::Framework12IntelGen13 => FrameworkPlatform::Framework12IntelGen13,
            Platform::IntelGen11 => FrameworkPlatform::IntelGen11,
            Platform::IntelGen12 => FrameworkPlatform::IntelGen12,
            Platform::IntelGen13 => FrameworkPlatform::IntelGen13,
            Platform::IntelCoreUltra1 => FrameworkPlatform::IntelCoreUltra1,
            Platform::IntelCoreUltra3 => FrameworkPlatform::IntelCoreUltra3,
            Platform::Framework13Amd7080 => FrameworkPlatform::Framework13Amd7080,
            Platform::Framework13AmdAi300 => FrameworkPlatform::Framework13AmdAi300,
            Platform::Framework16Amd7080 => FrameworkPlatform::Framework16Amd7080,
            Platform::Framework16AmdAi300 => FrameworkPlatform::Framework16AmdAi300,
            Platform::FrameworkDesktopAmdAiMax300 => FrameworkPlatform::FrameworkDesktopAmdAiMax300,
            Platform::GenericFramework(..) => FrameworkPlatform::GenericFramework,
            Platform::UnknownSystem => FrameworkPlatform::UnknownSystem,
        }
    }
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

impl From<PlatformFamily> for FrameworkPlatformFamily {
    fn from(value: PlatformFamily) -> Self {
        match value {
            PlatformFamily::Framework12 => FrameworkPlatformFamily::Framework12,
            PlatformFamily::Framework13 => FrameworkPlatformFamily::Framework13,
            PlatformFamily::Framework16 => FrameworkPlatformFamily::Framework16,
            PlatformFamily::FrameworkDesktop => FrameworkPlatformFamily::FrameworkDesktop,
        }
    }
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

impl From<ThermalSensorStatus> for FrameworkTemperatureState {
    fn from(value: ThermalSensorStatus) -> Self {
        match value {
            ThermalSensorStatus::Ok => FrameworkTemperatureState::Ok,
            ThermalSensorStatus::NotPresent => FrameworkTemperatureState::NotPresent,
            ThermalSensorStatus::Error => FrameworkTemperatureState::Error,
            ThermalSensorStatus::NotPowered => FrameworkTemperatureState::NotPowered,
            ThermalSensorStatus::NotCalibrated => FrameworkTemperatureState::NotCalibrated,
        }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcCurrentImage {
    Unknown = 0,
    Ro = 1,
    Rw = 2,
}

impl From<EcCurrentImage> for FrameworkEcCurrentImage {
    fn from(value: EcCurrentImage) -> Self {
        match value {
            EcCurrentImage::Unknown => FrameworkEcCurrentImage::Unknown,
            EcCurrentImage::RO => FrameworkEcCurrentImage::Ro,
            EcCurrentImage::RW => FrameworkEcCurrentImage::Rw,
        }
    }
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

impl Default for FrameworkByteBuffer {
    fn default() -> Self {
        Self {
            ptr: ptr::null_mut(),
            length: 0,
            capacity: 0,
        }
    }
}

impl FrameworkByteBuffer {
    fn from_vec(bytes: Vec<u8>) -> Self {
        let length = i32::try_from(bytes.len()).expect("buffer length overflowed i32");
        let capacity = i32::try_from(bytes.capacity()).expect("buffer capacity overflowed i32");
        let mut bytes = std::mem::ManuallyDrop::new(bytes);

        Self {
            ptr: bytes.as_mut_ptr(),
            length,
            capacity,
        }
    }

    unsafe fn destroy(self) {
        if self.ptr.is_null() {
            return;
        }

        let length = usize::try_from(self.length).expect("negative buffer length");
        let capacity = usize::try_from(self.capacity).expect("negative buffer capacity");
        drop(Vec::from_raw_parts(self.ptr, length, capacity));
    }
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

fn device_error_messages() -> &'static Mutex<VecDeque<(i32, String)>> {
    DEVICE_ERROR_MESSAGES.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn store_device_error_message(message: String) -> i32 {
    let id = NEXT_DEVICE_ERROR_ID.fetch_add(1, Ordering::Relaxed);
    let mut messages = device_error_messages()
        .lock()
        .expect("device error message lock poisoned");
    messages.push_back((id, message));
    while messages.len() > STORED_DEVICE_ERROR_LIMIT {
        messages.pop_front();
    }
    id
}

fn get_device_error_message(detail: i32) -> Option<String> {
    if detail <= 0 {
        return None;
    }

    let messages = device_error_messages().lock().ok()?;
    messages
        .iter()
        .find(|(id, _)| *id == detail)
        .map(|(_, message)| message.clone())
}

fn ec_response_detail_from_raw(detail: i32) -> FrameworkEcResponseDetail {
    match detail {
        0 => FrameworkEcResponseDetail::Success,
        1 => FrameworkEcResponseDetail::InvalidCommand,
        2 => FrameworkEcResponseDetail::Error,
        3 => FrameworkEcResponseDetail::InvalidParameter,
        4 => FrameworkEcResponseDetail::AccessDenied,
        5 => FrameworkEcResponseDetail::InvalidResponse,
        6 => FrameworkEcResponseDetail::InvalidVersion,
        7 => FrameworkEcResponseDetail::InvalidChecksum,
        8 => FrameworkEcResponseDetail::InProgress,
        9 => FrameworkEcResponseDetail::Unavailable,
        10 => FrameworkEcResponseDetail::Timeout,
        11 => FrameworkEcResponseDetail::Overflow,
        12 => FrameworkEcResponseDetail::InvalidHeader,
        13 => FrameworkEcResponseDetail::RequestTruncated,
        14 => FrameworkEcResponseDetail::ResponseTooBig,
        15 => FrameworkEcResponseDetail::BusError,
        16 => FrameworkEcResponseDetail::Busy,
        _ => FrameworkEcResponseDetail::Unknown,
    }
}
fn ec_response_detail_name(detail: FrameworkEcResponseDetail) -> &'static str {
    match detail {
        FrameworkEcResponseDetail::Unknown => "Unknown",
        FrameworkEcResponseDetail::Success => "Success",
        FrameworkEcResponseDetail::InvalidCommand => "InvalidCommand",
        FrameworkEcResponseDetail::Error => "Error",
        FrameworkEcResponseDetail::InvalidParameter => "InvalidParameter",
        FrameworkEcResponseDetail::AccessDenied => "AccessDenied",
        FrameworkEcResponseDetail::InvalidResponse => "InvalidResponse",
        FrameworkEcResponseDetail::InvalidVersion => "InvalidVersion",
        FrameworkEcResponseDetail::InvalidChecksum => "InvalidChecksum",
        FrameworkEcResponseDetail::InProgress => "InProgress",
        FrameworkEcResponseDetail::Unavailable => "Unavailable",
        FrameworkEcResponseDetail::Timeout => "Timeout",
        FrameworkEcResponseDetail::Overflow => "Overflow",
        FrameworkEcResponseDetail::InvalidHeader => "InvalidHeader",
        FrameworkEcResponseDetail::RequestTruncated => "RequestTruncated",
        FrameworkEcResponseDetail::ResponseTooBig => "ResponseTooBig",
        FrameworkEcResponseDetail::BusError => "BusError",
        FrameworkEcResponseDetail::Busy => "Busy",
    }
}

fn default_ec_flash_versions() -> FrameworkEcFlashVersions {
    FrameworkEcFlashVersions {
        current_image: FrameworkEcCurrentImage::Unknown,
        ro_version: FrameworkByteBuffer::default(),
        rw_version: FrameworkByteBuffer::default(),
    }
}

fn default_battery_snapshot() -> FrameworkBatterySnapshot {
    FrameworkBatterySnapshot {
        battery_state: FrameworkBatteryState::NotPresent,
        reserved: [0; 3],
        present_voltage: 0,
        present_rate: 0,
        remaining_capacity: 0,
        design_capacity: 0,
        design_voltage: 0,
        last_full_charge_capacity: 0,
        cycle_count: 0,
        charge_percentage: 0,
        manufacturer: FrameworkByteBuffer::default(),
        model_number: FrameworkByteBuffer::default(),
        serial_number: FrameworkByteBuffer::default(),
        battery_type: FrameworkByteBuffer::default(),
    }
}

fn default_power_snapshot() -> FrameworkPowerSnapshot {
    FrameworkPowerSnapshot {
        power_source_state: FrameworkPowerSourceState::None,
        battery_count: 0,
        reserved: [0; 2],
        battery_0: default_battery_snapshot(),
    }
}

fn default_fan_capabilities() -> FrameworkFanCapabilities {
    FrameworkFanCapabilities {
        fan_count: 0,
        features: FrameworkFanFeaturesState::None,
        reserved: [0; 2],
    }
}

fn default_feature_flags_result() -> FrameworkEcFeatureFlagsResult {
    FrameworkEcFeatureFlagsResult {
        status: FrameworkStatus::success(),
        flags: 0,
    }
}

fn default_keyboard_backlight_result() -> FrameworkEcKeyboardBacklightResult {
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

fn default_fingerprint_led_result() -> FrameworkEcFingerprintLedResult {
    FrameworkEcFingerprintLedResult {
        status: FrameworkStatus::success(),
        state: default_fingerprint_led_state(),
    }
}

fn default_expansion_bay_status() -> FrameworkEcExpansionBayStatus {
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

fn default_expansion_bay_status_result() -> FrameworkEcExpansionBayStatusResult {
    FrameworkEcExpansionBayStatusResult {
        status: FrameworkStatus::success(),
        bay: default_expansion_bay_status(),
    }
}

fn default_module_descriptor() -> FrameworkModuleDescriptor {
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
    }
}

fn default_module_inventory() -> FrameworkModuleInventory {
    let none = default_module_descriptor();
    FrameworkModuleInventory {
        usb_c_slot_count: 0,
        input_top_row_count: 0,
        detached_count: 0,
        reserved_0: 0,
        usb_c_slot_0: none,
        usb_c_slot_1: none,
        usb_c_slot_2: none,
        usb_c_slot_3: none,
        usb_c_slot_4: none,
        usb_c_slot_5: none,
        input_top_row_0: none,
        input_top_row_1: none,
        input_top_row_2: none,
        input_top_row_3: none,
        input_top_row_4: none,
        input_touchpad: none,
        internal_keyboard: none,
        internal_touchpad: none,
        fingerprint_reader: none,
        touchscreen: none,
        webcam: none,
        expansion_bay: none,
        detached_0: none,
        detached_1: none,
        detached_2: none,
        detached_3: none,
    }
}

fn default_module_inventory_result() -> FrameworkEcModuleInventoryResult {
    FrameworkEcModuleInventoryResult {
        status: FrameworkStatus::success(),
        inventory: default_module_inventory(),
    }
}

fn module_flag(flag: FrameworkModuleFlag) -> u32 {
    flag as u32
}

fn framework_feature_flag(flag: FrameworkEcFeatureFlag) -> u64 {
    flag as u64
}

fn fingerprint_led_level(level: Option<FpLedBrightnessLevel>) -> FrameworkFingerprintLedLevel {
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

fn expansion_bay_board(
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

fn expansion_bay_vendor(vendor: Option<GpuVendor>) -> FrameworkExpansionBayVendor {
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

fn gpu_pcie_config(config: Option<GpuPcieConfig>) -> FrameworkGpuPcieConfig {
    match config {
        Some(GpuPcieConfig::Pcie8x1) => FrameworkGpuPcieConfig::Unknown,
        Some(GpuPcieConfig::Pcie4x1) => FrameworkGpuPcieConfig::Pcie4x1,
        Some(GpuPcieConfig::Pcie4x2) => FrameworkGpuPcieConfig::Pcie4x2,
        None => FrameworkGpuPcieConfig::Unknown,
    }
}

#[allow(clippy::too_many_arguments)]
fn module_descriptor(
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

fn feature_flags_result(status: FrameworkStatus, flags: u64) -> FrameworkEcFeatureFlagsResult {
    FrameworkEcFeatureFlagsResult { status, flags }
}

fn keyboard_backlight_result(
    status: FrameworkStatus,
    brightness_percent: u8,
) -> FrameworkEcKeyboardBacklightResult {
    FrameworkEcKeyboardBacklightResult {
        status,
        brightness_percent,
        reserved: [0; 3],
    }
}

fn fingerprint_led_result(
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

fn expansion_bay_status_result(
    status: FrameworkStatus,
    bay: FrameworkEcExpansionBayStatus,
) -> FrameworkEcExpansionBayStatusResult {
    FrameworkEcExpansionBayStatusResult { status, bay }
}

fn module_inventory_result(
    status: FrameworkStatus,
    inventory: FrameworkModuleInventory,
) -> FrameworkEcModuleInventoryResult {
    FrameworkEcModuleInventoryResult { status, inventory }
}

#[derive(Clone, Copy, Debug, Default)]
struct PdPortObservation {
    connected: bool,
    has_pd_contract: bool,
    dp_alt_mode: bool,
    active: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct HidModuleObservation {
    vendor_id: u16,
    product_id: u16,
}

#[derive(Clone, Copy, Debug, Default)]
struct UsbModuleObservation {
    vendor_id: u16,
    product_id: u16,
    slot_index: i32,
}

fn detect_expansion_cards_local() -> Vec<HidModuleObservation> {
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

fn detect_audio_cards_local() -> Vec<UsbModuleObservation> {
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

fn detect_input_modules_local() -> Vec<UsbModuleObservation> {
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

fn detect_touchpads_local() -> Vec<HidModuleObservation> {
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

fn detect_touchscreens_local() -> Vec<HidModuleObservation> {
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

fn detect_cameras_local() -> Vec<UsbModuleObservation> {
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

fn read_pd_port_observation(ec: &CrosEc, port: u8) -> Option<PdPortObservation> {
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

fn unknown_usb_c_descriptor(
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

fn expansion_card_identity(product_id: u16) -> FrameworkModuleIdentity {
    match product_id {
        hid::DP_CARD_PID => FrameworkModuleIdentity::DpExpansionCard,
        hid::HDMI_CARD_PID => FrameworkModuleIdentity::HdmiExpansionCard,
        _ => FrameworkModuleIdentity::UnknownUsbCOccupant,
    }
}

fn framework16_top_row_identity(product_id: u16) -> FrameworkModuleIdentity {
    if product_id == inputmodule::LEDMATRIX_PID {
        FrameworkModuleIdentity::Framework16LedMatrix
    } else {
        FrameworkModuleIdentity::Framework16KeyboardModule
    }
}

fn input_deck_module_identity(module_type: InputModuleType) -> FrameworkModuleIdentity {
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

fn expansion_bay_identity(
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

fn push_detached_module(
    detached: &mut [FrameworkModuleDescriptor; 4],
    detached_count: &mut u8,
    descriptor: FrameworkModuleDescriptor,
) {
    if let Some(slot) = detached.get_mut(*detached_count as usize) {
        *slot = descriptor;
        *detached_count += 1;
    }
}

fn expansion_bay_status(ec: &CrosEc) -> Result<FrameworkEcExpansionBayStatus, FrameworkStatus> {
    let info = EcRequestExpansionBayStatus {}
        .send_command(ec)
        .map_err(status_from_error)?;
    let gpu = EcRequestGetGpuPcie {}
        .send_command(ec)
        .map_err(status_from_error)?;

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

fn feature_flags(ec: &CrosEc) -> Result<u64, FrameworkStatus> {
    let mut flags = 0u64;

    if feature_enabled(ec, EcFeatureCode::Keyboard)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::Keyboard);
    }
    if feature_enabled(ec, EcFeatureCode::PwmKeyboardBacklight)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::KeyboardBacklight);
    }
    if feature_enabled(ec, EcFeatureCode::Touchpad)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::Touchpad);
    }
    if feature_enabled(ec, EcFeatureCode::Fingerprint)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::Fingerprint);
    }
    if feature_enabled(ec, EcFeatureCode::MotionSense)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::AmbientLight);
    }
    if feature_enabled(ec, EcFeatureCode::MotionSense)? {
        flags |= framework_feature_flag(FrameworkEcFeatureFlag::TabletMode);
    }

    Ok(flags)
}

fn build_module_inventory(handle: &FrameworkEcHandle) -> FrameworkModuleInventory {
    let mut usb_slots = [default_module_descriptor(); MAX_USB_C_SLOT_COUNT];
    let mut top_row = [default_module_descriptor(); 5];
    let mut detached = [default_module_descriptor(); 4];
    let mut detached_count = 0u8;

    let family = smbios::get_family();
    let usb_c_slot_count = if family == Some(PlatformFamily::Framework16) {
        6
    } else {
        4
    };
    let mut input_touchpad = default_module_descriptor();
    let mut internal_keyboard = default_module_descriptor();
    let mut internal_touchpad = default_module_descriptor();
    let mut fingerprint_reader = default_module_descriptor();
    let mut touchscreen_module = default_module_descriptor();
    let mut webcam = default_module_descriptor();
    let mut expansion_bay_module = default_module_descriptor();

    let mut pd_observations = [None; MAX_USB_C_SLOT_COUNT];
    for (index, slot) in usb_slots
        .iter_mut()
        .enumerate()
        .take(usb_c_slot_count as usize)
    {
        pd_observations[index] = read_pd_port_observation(&handle.ec, index as u8);
        if let Some(observation) = pd_observations[index] {
            if let Some(descriptor) = unknown_usb_c_descriptor(index, observation) {
                *slot = descriptor;
            }
        }
    }

    let expansion_cards = detect_expansion_cards_local();
    let dp_slots: Vec<usize> = pd_observations
        .iter()
        .enumerate()
        .filter_map(|(index, observation)| observation.filter(|obs| obs.dp_alt_mode).map(|_| index))
        .collect();

    if expansion_cards.len() == 1 && dp_slots.len() == 1 {
        let card = &expansion_cards[0];
        let slot_index = dp_slots[0];
        let mut flags = usb_slots[slot_index].flags;
        flags |= module_flag(FrameworkModuleFlag::Connected);
        usb_slots[slot_index] = module_descriptor(
            expansion_card_identity(card.product_id),
            FrameworkModuleBus::Composite,
            FrameworkModuleSlotKind::UsbCPort,
            FrameworkModuleConfidence::DerivedStrong,
            true,
            slot_index as i32,
            flags,
            card.vendor_id as u32,
            card.product_id as u32,
            -1,
        );
    } else {
        for card in &expansion_cards {
            push_detached_module(
                &mut detached,
                &mut detached_count,
                module_descriptor(
                    expansion_card_identity(card.product_id),
                    FrameworkModuleBus::Hid,
                    FrameworkModuleSlotKind::Detached,
                    FrameworkModuleConfidence::Direct,
                    true,
                    -1,
                    module_flag(FrameworkModuleFlag::Connected),
                    card.vendor_id as u32,
                    card.product_id as u32,
                    -1,
                ),
            );
        }
    }

    let audio_cards = detect_audio_cards_local();
    let audio_candidates: Vec<usize> = pd_observations
        .iter()
        .enumerate()
        .filter_map(|(index, observation)| {
            observation.and_then(|obs| {
                if obs.connected
                    && !obs.dp_alt_mode
                    && matches!(
                        usb_slots[index].identity,
                        FrameworkModuleIdentity::UnknownUsbCOccupant
                    )
                {
                    Some(index)
                } else {
                    None
                }
            })
        })
        .collect();

    if audio_cards.len() == 1 && audio_candidates.len() == 1 {
        let card = &audio_cards[0];
        let slot_index = audio_candidates[0];
        let mut flags = usb_slots[slot_index].flags;
        flags |= module_flag(FrameworkModuleFlag::Connected);
        usb_slots[slot_index] = module_descriptor(
            FrameworkModuleIdentity::AudioExpansionCard,
            FrameworkModuleBus::Composite,
            FrameworkModuleSlotKind::UsbCPort,
            FrameworkModuleConfidence::DerivedWeak,
            true,
            slot_index as i32,
            flags,
            card.vendor_id as u32,
            card.product_id as u32,
            -1,
        );
    } else {
        for card in &audio_cards {
            push_detached_module(
                &mut detached,
                &mut detached_count,
                module_descriptor(
                    FrameworkModuleIdentity::AudioExpansionCard,
                    FrameworkModuleBus::Usb,
                    FrameworkModuleSlotKind::Detached,
                    FrameworkModuleConfidence::Direct,
                    true,
                    -1,
                    module_flag(FrameworkModuleFlag::Connected),
                    card.vendor_id as u32,
                    card.product_id as u32,
                    -1,
                ),
            );
        }
    }

    let input_top_row_count = if family == Some(PlatformFamily::Framework16) {
        5
    } else {
        0
    };
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
    } else {
        let keyboard_present =
            feature_enabled(&handle.ec, EcFeatureCode::Keyboard).unwrap_or(false);
        if keyboard_present {
            internal_keyboard = module_descriptor(
                FrameworkModuleIdentity::InternalKeyboard,
                FrameworkModuleBus::Ec,
                FrameworkModuleSlotKind::InternalFixed,
                FrameworkModuleConfidence::DerivedStrong,
                true,
                -1,
                module_flag(FrameworkModuleFlag::BuiltIn)
                    | module_flag(FrameworkModuleFlag::Connected),
                0,
                0,
                -1,
            );
        }
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
        let mut present = feature_enabled(&handle.ec, EcFeatureCode::Touchpad).unwrap_or(false);
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

    if feature_enabled(&handle.ec, EcFeatureCode::Fingerprint).unwrap_or(false)
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

    FrameworkModuleInventory {
        usb_c_slot_count,
        input_top_row_count,
        detached_count,
        reserved_0: 0,
        usb_c_slot_0: usb_slots[0],
        usb_c_slot_1: usb_slots[1],
        usb_c_slot_2: usb_slots[2],
        usb_c_slot_3: usb_slots[3],
        usb_c_slot_4: usb_slots[4],
        usb_c_slot_5: usb_slots[5],
        input_top_row_0: top_row[0],
        input_top_row_1: top_row[1],
        input_top_row_2: top_row[2],
        input_top_row_3: top_row[3],
        input_top_row_4: top_row[4],
        input_touchpad,
        internal_keyboard,
        internal_touchpad,
        fingerprint_reader,
        touchscreen: touchscreen_module,
        webcam,
        expansion_bay: expansion_bay_module,
        detached_0: detached[0],
        detached_1: detached[1],
        detached_2: detached[2],
        detached_3: detached[3],
    }
}

fn fan_features_state(
    supports_fan_control: bool,
    supports_thermal_reporting: bool,
) -> FrameworkFanFeaturesState {
    match (supports_fan_control, supports_thermal_reporting) {
        (false, false) => FrameworkFanFeaturesState::None,
        (true, false) => FrameworkFanFeaturesState::FanControl,
        (false, true) => FrameworkFanFeaturesState::ThermalReporting,
        (true, true) => FrameworkFanFeaturesState::All,
    }
}

fn power_source_state(ac_present: bool, battery_present: bool) -> FrameworkPowerSourceState {
    match (ac_present, battery_present) {
        (false, false) => FrameworkPowerSourceState::None,
        (true, false) => FrameworkPowerSourceState::AcOnly,
        (false, true) => FrameworkPowerSourceState::BatteryOnly,
        (true, true) => FrameworkPowerSourceState::AcAndBattery,
    }
}

fn battery_state(level_critical: bool, discharging: bool, charging: bool) -> FrameworkBatteryState {
    if level_critical {
        FrameworkBatteryState::Critical
    } else {
        match (discharging, charging) {
            (false, false) => FrameworkBatteryState::Idle,
            (false, true) => FrameworkBatteryState::Charging,
            (true, false) => FrameworkBatteryState::Discharging,
            (true, true) => FrameworkBatteryState::ChargingAndDischarging,
        }
    }
}

fn default_temperature_reading() -> FrameworkTemperatureReading {
    FrameworkTemperatureReading {
        state: FrameworkTemperatureState::NotPresent,
        celsius: 0,
        reserved: 0,
    }
}

fn default_fan_reading() -> FrameworkFanReading {
    FrameworkFanReading {
        state: FrameworkFanState::NotPresent,
        rpm: 0,
        reserved: 0,
    }
}

fn fan_state(present: bool, stalled: bool) -> FrameworkFanState {
    if !present {
        FrameworkFanState::NotPresent
    } else if stalled {
        FrameworkFanState::Stalled
    } else {
        FrameworkFanState::Ok
    }
}

fn default_thermal_snapshot() -> FrameworkThermalSnapshot {
    let temperature = default_temperature_reading();
    let fan = default_fan_reading();

    FrameworkThermalSnapshot {
        fan_count: 0,
        reserved: [0; 3],
        temperature_0: temperature,
        temperature_1: temperature,
        temperature_2: temperature,
        temperature_3: temperature,
        temperature_4: temperature,
        temperature_5: temperature,
        temperature_6: temperature,
        temperature_7: temperature,
        fan_0: fan,
        fan_1: fan,
        fan_2: fan,
        fan_3: fan,
    }
}

fn ec_handle_result(
    status: FrameworkStatus,
    handle: *mut FrameworkEcHandle,
) -> FrameworkEcHandleResult {
    FrameworkEcHandleResult { status, handle }
}

fn product_name_result(
    status: FrameworkStatus,
    product_name: FrameworkByteBuffer,
) -> FrameworkProductNameResult {
    FrameworkProductNameResult {
        status,
        product_name,
    }
}

fn build_info_result(
    status: FrameworkStatus,
    build_info: FrameworkByteBuffer,
) -> FrameworkEcBuildInfoResult {
    FrameworkEcBuildInfoResult { status, build_info }
}

fn flash_versions_result(
    status: FrameworkStatus,
    versions: FrameworkEcFlashVersions,
) -> FrameworkEcFlashVersionsResult {
    FrameworkEcFlashVersionsResult { status, versions }
}

fn power_snapshot_result(
    status: FrameworkStatus,
    snapshot: FrameworkPowerSnapshot,
) -> FrameworkEcPowerSnapshotResult {
    FrameworkEcPowerSnapshotResult { status, snapshot }
}

fn fan_capabilities_result(
    status: FrameworkStatus,
    capabilities: FrameworkFanCapabilities,
) -> FrameworkEcFanCapabilitiesResult {
    FrameworkEcFanCapabilitiesResult {
        status,
        capabilities,
    }
}

fn thermal_snapshot_result(
    status: FrameworkStatus,
    snapshot: FrameworkThermalSnapshot,
) -> FrameworkEcThermalSnapshotResult {
    FrameworkEcThermalSnapshotResult { status, snapshot }
}

fn active_driver_result(
    status: FrameworkStatus,
    driver: FrameworkEcDriver,
) -> FrameworkEcActiveDriverResult {
    FrameworkEcActiveDriverResult { status, driver }
}

fn status_device_error_message_result(
    status: FrameworkStatus,
    message: FrameworkByteBuffer,
) -> FrameworkStatusDeviceErrorMessageResult {
    FrameworkStatusDeviceErrorMessageResult { status, message }
}

fn status_description_result(
    status: FrameworkStatus,
    description: FrameworkByteBuffer,
) -> FrameworkStatusDescriptionResult {
    FrameworkStatusDescriptionResult {
        status,
        description,
    }
}

fn platform_result(
    status: FrameworkStatus,
    platform: FrameworkPlatform,
) -> FrameworkPlatformResult {
    FrameworkPlatformResult { status, platform }
}

fn platform_family_result(
    status: FrameworkStatus,
    family: FrameworkPlatformFamily,
) -> FrameworkPlatformFamilyResult {
    FrameworkPlatformFamilyResult { status, family }
}

fn set_fan_rpm_result(
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

fn set_fan_duty_result(
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

fn restore_auto_fan_control_result(
    status: FrameworkStatus,
    fan_index: i32,
) -> FrameworkEcRestoreAutoFanControlResult {
    FrameworkEcRestoreAutoFanControlResult { status, fan_index }
}

fn status_description(status: FrameworkStatus) -> String {
    match status.code {
        FrameworkStatusCode::Success => "Success".to_string(),
        FrameworkStatusCode::NullPointer => "Null pointer".to_string(),
        FrameworkStatusCode::InvalidArgument => {
            format!(
                "Invalid fan index: {}",
                status.invalid_fan_index_value().unwrap_or_default()
            )
        }
        FrameworkStatusCode::NoDriverAvailable => "No EC driver available".to_string(),
        FrameworkStatusCode::UnsupportedDriver => {
            "Requested EC driver is not supported on this system".to_string()
        }
        FrameworkStatusCode::DeviceError => {
            if let Some(message) = status
                .device_error_message_token()
                .and_then(get_device_error_message)
            {
                format!("Device error: {}", message)
            } else {
                "Device error".to_string()
            }
        }
        FrameworkStatusCode::EcResponse => {
            let detail = status
                .ec_response_detail()
                .unwrap_or(FrameworkEcResponseDetail::Unknown);
            format!(
                "EC response: {} ({})",
                ec_response_detail_name(detail),
                detail as i32
            )
        }
        FrameworkStatusCode::UnknownResponseCode => {
            format!(
                "Unknown EC response code: {}",
                status.unknown_response_code_value().unwrap_or_default()
            )
        }
        FrameworkStatusCode::DataUnavailable => "Data unavailable".to_string(),
    }
}

fn status_from_error(error: EcError) -> FrameworkStatus {
    match error {
        EcError::Response(response) => {
            FrameworkStatus::with(FrameworkStatusCode::EcResponse, response as i32)
        }
        EcError::UnknownResponseCode(code) => FrameworkStatus::with(
            FrameworkStatusCode::UnknownResponseCode,
            i32::try_from(code).unwrap_or(i32::MAX),
        ),
        EcError::DeviceError(message) => {
            let detail = store_device_error_message(message);
            FrameworkStatus::with(FrameworkStatusCode::DeviceError, detail)
        }
    }
}

fn default_ec_handle() -> Option<FrameworkEcHandle> {
    #[cfg(windows)]
    if let Some(ec) = CrosEc::with(CrosEcDriverType::Windows) {
        return Some(FrameworkEcHandle {
            ec,
            driver: FrameworkEcDriver::Windows,
        });
    }

    #[cfg(target_os = "linux")]
    if std::path::Path::new(CROS_EC_DEV_PATH).exists() {
        if let Some(ec) = CrosEc::with(CrosEcDriverType::CrosEc) {
            return Some(FrameworkEcHandle {
                ec,
                driver: FrameworkEcDriver::CrosEc,
            });
        }
    }

    #[cfg(all(not(windows), target_arch = "x86_64"))]
    if let Some(ec) = CrosEc::with(CrosEcDriverType::Portio) {
        return Some(FrameworkEcHandle {
            ec,
            driver: FrameworkEcDriver::Portio,
        });
    }

    None
}

fn read_feature_flags(ec: &CrosEc) -> Result<[u32; 2], FrameworkStatus> {
    EcRequestGetFeatures {}
        .send_command(ec)
        .map(|response| response.flags)
        .map_err(status_from_error)
}

fn feature_enabled(ec: &CrosEc, feature: EcFeatureCode) -> Result<bool, FrameworkStatus> {
    let flags = read_feature_flags(ec)?;
    let index = feature as usize;
    let word = index / 32;
    let bit = index % 32;
    Ok((flags[word] & (1 << bit)) != 0)
}

fn require_handle<'a>(
    handle: *const FrameworkEcHandle,
) -> Result<&'a FrameworkEcHandle, FrameworkStatus> {
    if handle.is_null() {
        return Err(FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0));
    }

    // SAFETY: the caller guarantees the handle pointer came from framework_ec_open_*.
    Ok(unsafe { &*handle })
}

fn parse_optional_fan_index(fan_index: i32) -> Result<Option<u32>, FrameworkStatus> {
    if fan_index == -1 {
        return Ok(None);
    }

    let fan_index = u32::try_from(fan_index)
        .map_err(|_| FrameworkStatus::with(FrameworkStatusCode::InvalidArgument, fan_index))?;
    Ok(Some(fan_index))
}

fn parse_optional_fan_index_u8(fan_index: i32) -> Result<Option<u8>, FrameworkStatus> {
    if fan_index == -1 {
        return Ok(None);
    }

    let fan_index = u8::try_from(fan_index)
        .map_err(|_| FrameworkStatus::with(FrameworkStatusCode::InvalidArgument, fan_index))?;
    Ok(Some(fan_index))
}

#[no_mangle]
pub extern "C" fn framework_ec_driver_is_supported(driver: FrameworkEcDriver) -> bool {
    let Ok(driver) = CrosEcDriverType::try_from(driver) else {
        return false;
    };

    CrosEc::with(driver).is_some()
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
    let Some(handle) = default_ec_handle() else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::NoDriverAvailable, 0),
            ptr::null_mut(),
        );
    };

    if let Err(error) = handle.ec.check_mem_magic() {
        return ec_handle_result(status_from_error(error), ptr::null_mut());
    }

    ec_handle_result(FrameworkStatus::success(), Box::into_raw(Box::new(handle)))
}

#[no_mangle]
pub extern "C" fn framework_ec_open_with_driver(
    driver: FrameworkEcDriver,
) -> FrameworkEcHandleResult {
    let Ok(driver_type) = CrosEcDriverType::try_from(driver) else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::UnsupportedDriver, 0),
            ptr::null_mut(),
        );
    };

    let Some(ec) = CrosEc::with(driver_type) else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::UnsupportedDriver, 0),
            ptr::null_mut(),
        );
    };

    if let Err(error) = ec.check_mem_magic() {
        return ec_handle_result(status_from_error(error), ptr::null_mut());
    }

    ec_handle_result(
        FrameworkStatus::success(),
        Box::into_raw(Box::new(FrameworkEcHandle { ec, driver })),
    )
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
        Err(status) => return power_snapshot_result(status, default_power_snapshot()),
    };

    let Some(power_info) = power::power_info(&handle.ec) else {
        return power_snapshot_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            default_power_snapshot(),
        );
    };

    let mut snapshot = FrameworkPowerSnapshot {
        power_source_state: power_source_state(power_info.ac_present, power_info.battery.is_some()),
        ..default_power_snapshot()
    };

    if let Some(battery) = power_info.battery {
        snapshot.battery_count = battery.battery_count;
        snapshot.battery_0 = FrameworkBatterySnapshot {
            battery_state: battery_state(
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
        Err(status) => return fan_capabilities_result(status, default_fan_capabilities()),
    };

    let fan_control = match feature_enabled(&handle.ec, EcFeatureCode::PwmFan) {
        Ok(supported) => supported,
        Err(status) => return fan_capabilities_result(status, default_fan_capabilities()),
    };
    let thermal = match feature_enabled(&handle.ec, EcFeatureCode::Thermal) {
        Ok(supported) => supported,
        Err(status) => return fan_capabilities_result(status, default_fan_capabilities()),
    };

    let fan_count = thermal_snapshot(&handle.ec)
        .map(|snapshot| snapshot.fan_count)
        .unwrap_or(0);

    fan_capabilities_result(
        FrameworkStatus::success(),
        FrameworkFanCapabilities {
            fan_count,
            features: fan_features_state(fan_control, thermal),
            reserved: [0; 2],
        },
    )
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_thermal_snapshot(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcThermalSnapshotResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return thermal_snapshot_result(status, default_thermal_snapshot()),
    };

    let Some(snapshot) = thermal_snapshot(&handle.ec) else {
        return thermal_snapshot_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            default_thermal_snapshot(),
        );
    };

    let mut temperatures = [default_temperature_reading(); THERMAL_SENSOR_COUNT];
    for (index, reading) in snapshot.temperatures.iter().enumerate() {
        temperatures[index] = FrameworkTemperatureReading {
            state: reading.status.into(),
            celsius: reading.celsius,
            reserved: 0,
        };
    }

    let mut fan_present = [0u8; FAN_SLOT_COUNT];
    let mut fan_stalled = [0u8; FAN_SLOT_COUNT];
    for index in 0..FAN_SLOT_COUNT {
        fan_present[index] = u8::from(snapshot.fan_present[index]);
        fan_stalled[index] = u8::from(snapshot.fan_stalled[index]);
    }

    thermal_snapshot_result(
        FrameworkStatus::success(),
        FrameworkThermalSnapshot {
            fan_count: snapshot.fan_count,
            reserved: [0; 3],
            temperature_0: temperatures[0],
            temperature_1: temperatures[1],
            temperature_2: temperatures[2],
            temperature_3: temperatures[3],
            temperature_4: temperatures[4],
            temperature_5: temperatures[5],
            temperature_6: temperatures[6],
            temperature_7: temperatures[7],
            fan_0: FrameworkFanReading {
                state: fan_state(snapshot.fan_present[0], snapshot.fan_stalled[0]),
                rpm: snapshot.fan_rpms[0],
                reserved: 0,
            },
            fan_1: FrameworkFanReading {
                state: fan_state(snapshot.fan_present[1], snapshot.fan_stalled[1]),
                rpm: snapshot.fan_rpms[1],
                reserved: 0,
            },
            fan_2: FrameworkFanReading {
                state: fan_state(snapshot.fan_present[2], snapshot.fan_stalled[2]),
                rpm: snapshot.fan_rpms[2],
                reserved: 0,
            },
            fan_3: FrameworkFanReading {
                state: fan_state(snapshot.fan_present[3], snapshot.fan_stalled[3]),
                rpm: snapshot.fan_rpms[3],
                reserved: 0,
            },
        },
    )
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
            let mut result = default_feature_flags_result();
            result.status = status;
            return result;
        }
    };

    match feature_flags(&handle.ec) {
        Ok(flags) => feature_flags_result(FrameworkStatus::success(), flags),
        Err(status) => feature_flags_result(status, 0),
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
            let mut result = default_keyboard_backlight_result();
            result.status = status;
            return result;
        }
    };

    match handle.ec.get_keyboard_backlight() {
        Ok(level) => keyboard_backlight_result(FrameworkStatus::success(), level),
        Err(error) => keyboard_backlight_result(status_from_error(error), 0),
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
            let mut result = default_fingerprint_led_result();
            result.status = status;
            return result;
        }
    };

    match handle.ec.get_fp_led_level() {
        Ok((raw_level, level)) => fingerprint_led_result(
            FrameworkStatus::success(),
            raw_level,
            fingerprint_led_level(level),
        ),
        Err(error) => fingerprint_led_result(
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
            let mut result = default_expansion_bay_status_result();
            result.status = status;
            return result;
        }
    };

    match expansion_bay_status(&handle.ec) {
        Ok(bay) => expansion_bay_status_result(FrameworkStatus::success(), bay),
        Err(status) => expansion_bay_status_result(status, default_expansion_bay_status()),
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
            let mut result = default_module_inventory_result();
            result.status = status;
            return result;
        }
    };

    module_inventory_result(FrameworkStatus::success(), build_module_inventory(handle))
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
