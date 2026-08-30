use framework_lib::chromium_ec::commands::{
    DeckStateMode, EcRequestGetGpuSerial, EcRequestGetUptimeInfo, EcRequestGpioGetV1Count,
    EcRequestGpioGetV1Info, EcRequestReadBoardId, EcRequestSetTabletMode, GpioGetSubCommand,
    MotionSenseChip, MotionSenseInfo, MotionSenseLocation, MotionSenseType, RgbS,
};
use framework_lib::chromium_ec::{CrosEc, CrosEcDriver, EcError, EcRequestRaw};

use crate::{
    FrameworkAccelDataResult, FrameworkDeckStateMode, FrameworkSensorCategory, FrameworkSensorChip,
    FrameworkSensorDescriptor, FrameworkSensorLocation, FrameworkSensorType, FrameworkStatus,
    FrameworkTabletModeOverride,
};

pub(crate) struct UptimeInfo {
    pub time_since_ec_boot_ms: u32,
    pub ap_resets_since_ec_boot: u32,
    pub ec_reset_flags: u32,
}

pub(crate) fn get_uptime(ec: &CrosEc) -> Result<UptimeInfo, EcError> {
    let res = EcRequestGetUptimeInfo {}.send_command(ec)?;
    Ok(UptimeInfo {
        time_since_ec_boot_ms: res.time_since_ec_boot,
        ap_resets_since_ec_boot: res.ap_resets_since_ec_boot,
        ec_reset_flags: res.ec_reset_flags,
    })
}

pub(crate) fn set_tablet_mode(
    ec: &CrosEc,
    mode: FrameworkTabletModeOverride,
) -> Result<(), EcError> {
    EcRequestSetTabletMode { mode: mode as u8 }.send_command(ec)?;
    Ok(())
}

pub(crate) fn into_deck_state_mode(mode: FrameworkDeckStateMode) -> DeckStateMode {
    match mode {
        FrameworkDeckStateMode::ReadOnly => DeckStateMode::ReadOnly,
        FrameworkDeckStateMode::Required => DeckStateMode::Required,
        FrameworkDeckStateMode::ForceOn => DeckStateMode::ForceOn,
        FrameworkDeckStateMode::ForceOff => DeckStateMode::ForceOff,
    }
}

pub(crate) fn read_board_id(ec: &CrosEc, board_id_type: u8) -> Result<i8, EcError> {
    let res = EcRequestReadBoardId { board_id_type }.send_command(ec)?;
    Ok(res.board_id)
}

const EC_MEMMAP_ACC_DATA: u16 = 0x92;
const EC_MEMMAP_ALS: u16 = 0x80;
const LID_ANGLE_UNRELIABLE: u16 = 500;

pub(crate) fn get_accel_data(ec: &CrosEc) -> Option<FrameworkAccelDataResult> {
    let lid_raw = ec.read_memory(EC_MEMMAP_ACC_DATA, 2)?;
    let base_raw = ec.read_memory(EC_MEMMAP_ACC_DATA + 2, 6)?;
    let lid_accel_raw = ec.read_memory(EC_MEMMAP_ACC_DATA + 8, 6)?;

    let lid_angle_raw = u16::from_le_bytes([lid_raw[0], lid_raw[1]]);
    let lid_angle_degrees = if lid_angle_raw == LID_ANGLE_UNRELIABLE {
        -1i16
    } else {
        lid_angle_raw as i16
    };

    Some(FrameworkAccelDataResult {
        status: FrameworkStatus::success(),
        lid_angle_degrees,
        reserved: [0; 2],
        base_x: i16::from_le_bytes([base_raw[0], base_raw[1]]),
        base_y: i16::from_le_bytes([base_raw[2], base_raw[3]]),
        base_z: i16::from_le_bytes([base_raw[4], base_raw[5]]),
        lid_x: i16::from_le_bytes([lid_accel_raw[0], lid_accel_raw[1]]),
        lid_y: i16::from_le_bytes([lid_accel_raw[2], lid_accel_raw[3]]),
        lid_z: i16::from_le_bytes([lid_accel_raw[4], lid_accel_raw[5]]),
    })
}

// EC_MEMMAP_ALS = 0x80: two 16-bit lux readings (4 bytes total).
// get_als_reading() from power.rs only works safely with index 0; read directly instead.
pub(crate) fn get_als(ec: &CrosEc) -> Option<(u32, u32)> {
    let als = ec.read_memory(EC_MEMMAP_ALS, 4)?;
    let lux_0 = u16::from_le_bytes([als[0], als[1]]) as u32;
    let lux_1 = u16::from_le_bytes([als[2], als[3]]) as u32;
    Some((lux_0, lux_1))
}

pub(crate) fn into_sensor_descriptor(info: &MotionSenseInfo) -> FrameworkSensorDescriptor {
    let st = sensor_type(&info.sensor_type);
    FrameworkSensorDescriptor {
        category: sensor_category(st),
        sensor_type: st,
        location: sensor_location(&info.location),
        chip: sensor_chip(&info.chip),
    }
}

fn sensor_category(t: FrameworkSensorType) -> FrameworkSensorCategory {
    match t {
        FrameworkSensorType::Accel | FrameworkSensorType::Gyro | FrameworkSensorType::Mag => {
            FrameworkSensorCategory::Motion
        }
        FrameworkSensorType::Light
        | FrameworkSensorType::LightRgb
        | FrameworkSensorType::Prox
        | FrameworkSensorType::Baro => FrameworkSensorCategory::Environmental,
        FrameworkSensorType::Activity | FrameworkSensorType::Sync => FrameworkSensorCategory::Other,
        FrameworkSensorType::Unknown => FrameworkSensorCategory::Unknown,
    }
}

fn sensor_type(t: &MotionSenseType) -> FrameworkSensorType {
    match t {
        MotionSenseType::Accel => FrameworkSensorType::Accel,
        MotionSenseType::Gyro => FrameworkSensorType::Gyro,
        MotionSenseType::Mag => FrameworkSensorType::Mag,
        MotionSenseType::Prox => FrameworkSensorType::Prox,
        MotionSenseType::Light => FrameworkSensorType::Light,
        MotionSenseType::Activity => FrameworkSensorType::Activity,
        MotionSenseType::Baro => FrameworkSensorType::Baro,
        MotionSenseType::Sync => FrameworkSensorType::Sync,
        MotionSenseType::LightRgb => FrameworkSensorType::LightRgb,
    }
}

fn sensor_location(l: &MotionSenseLocation) -> FrameworkSensorLocation {
    match l {
        MotionSenseLocation::Base => FrameworkSensorLocation::Base,
        MotionSenseLocation::Lid => FrameworkSensorLocation::Lid,
        MotionSenseLocation::Camera => FrameworkSensorLocation::Camera,
    }
}

/// Reads a raw ADC channel value.
pub(crate) fn adc_read(ec: &CrosEc, channel: u8) -> Result<i32, EcError> {
    ec.adc_read(channel)
}

/// Probes whether the EC implements a given host command at a given version.
///
/// Worth calling before any of the newer commands: support varies by platform and
/// EC firmware revision, and this distinguishes "not implemented" from "failed".
pub(crate) fn command_version_supported(
    ec: &CrosEc,
    command: u32,
    version: u8,
) -> Result<bool, EcError> {
    ec.cmd_version_supported(command, version)
}

pub(crate) fn get_hibernate_delay(ec: &CrosEc) -> Result<u32, EcError> {
    ec.get_ec_hib_delay()
}

pub(crate) fn set_hibernate_delay(ec: &CrosEc, seconds: u32) -> Result<(), EcError> {
    ec.set_ec_hib_delay(seconds)
}

/// Sets the charge rate limit.
///
/// `rate` is in amps and `battery_soc` an optional state-of-charge threshold in
/// percent above which the limit applies.
pub(crate) fn set_charge_rate_limit(
    ec: &CrosEc,
    rate: f32,
    battery_soc: Option<f32>,
) -> Result<(), EcError> {
    ec.set_charge_rate_limit(rate, battery_soc)
}

/// Sets the fingerprint LED brightness as a percentage, the finer-grained
/// counterpart to the discrete level control.
pub(crate) fn set_fp_led_percentage(ec: &CrosEc, percentage: u8) -> Result<(), EcError> {
    ec.set_fp_led_percentage(percentage)
}

pub(crate) fn ps2_emulation_enable(ec: &CrosEc, enable: bool) -> Result<(), EcError> {
    ec.ps2_emulation_enable(enable)
}

pub(crate) fn remap_key(ec: &CrosEc, row: u8, col: u8, scanset: u16) -> Result<(), EcError> {
    ec.remap_key(row, col, scanset)
}

pub(crate) fn remap_caps_to_ctrl(ec: &CrosEc) -> Result<(), EcError> {
    ec.remap_caps_to_ctrl()
}

pub(crate) fn set_rgb_keyboard_colors(
    ec: &CrosEc,
    start_key: u8,
    colors: Vec<RgbS>,
) -> Result<(), EcError> {
    ec.rgbkbd_set_color(start_key, colors)
}

pub(crate) fn get_gpio(ec: &CrosEc, name: &str) -> Result<bool, EcError> {
    ec.get_gpio(name)
}

pub(crate) fn set_gpio(ec: &CrosEc, name: &str, value: bool) -> Result<(), EcError> {
    ec.set_gpio(name, value)
}

pub(crate) struct GpioInfo {
    pub name: String,
    pub value: bool,
    pub flags: u32,
}

/// Number of GPIOs the EC exposes.
///
/// Upstream's `get_all_gpios` prints each entry and returns only the count, so
/// drive the Count/Info subcommands directly to get names and values back.
pub(crate) fn gpio_count(ec: &CrosEc) -> Result<u8, EcError> {
    Ok(EcRequestGpioGetV1Count {
        subcmd: GpioGetSubCommand::Count as u8,
    }
    .send_command(ec)?
    .val)
}

/// Reads one GPIO's name, level and flags by index.
pub(crate) fn gpio_info_at(ec: &CrosEc, index: u8) -> Result<GpioInfo, EcError> {
    let info = EcRequestGpioGetV1Info {
        subcmd: GpioGetSubCommand::Info as u8,
        index,
    }
    .send_command(ec)?;
    let name_bytes = { info.name };
    let name = std::str::from_utf8(&name_bytes)
        .unwrap_or_default()
        .trim_end_matches(char::from(0))
        .to_string();
    Ok(GpioInfo {
        name,
        value: { info.val } == 1,
        flags: { info.flags },
    })
}

/// Reads the expansion-bay GPU serial.
///
/// Sends the command directly rather than calling `CrosEc::get_gpu_serial`, whose
/// `String::from_utf8(..).unwrap()` would abort the host process on a garbled
/// serial. Invalid UTF-8 is reported as an error instead.
pub(crate) fn get_gpu_serial(ec: &CrosEc) -> Result<String, EcError> {
    let response = EcRequestGetGpuSerial { idx: 0 }.send_command(ec)?;
    if { response.valid } == 0 {
        return Err(EcError::DeviceError("No valid GPU serial".to_string()));
    }
    let serial = { response.serial };
    std::str::from_utf8(&serial)
        .map(|s| s.trim_end_matches(char::from(0)).trim().to_string())
        .map_err(|err| EcError::DeviceError(format!("GPU serial is not valid UTF-8: {:?}", err)))
}

fn sensor_chip(c: &MotionSenseChip) -> FrameworkSensorChip {
    match c {
        MotionSenseChip::Kxcj9 => FrameworkSensorChip::Kxcj9,
        MotionSenseChip::Lsm6ds0 => FrameworkSensorChip::Lsm6ds0,
        MotionSenseChip::Bmi160 => FrameworkSensorChip::Bmi160,
        MotionSenseChip::Si1141 => FrameworkSensorChip::Si1141,
        MotionSenseChip::Si1142 => FrameworkSensorChip::Si1142,
        MotionSenseChip::Si1143 => FrameworkSensorChip::Si1143,
        MotionSenseChip::Kx022 => FrameworkSensorChip::Kx022,
        MotionSenseChip::L3gd20h => FrameworkSensorChip::L3gd20h,
        MotionSenseChip::Bma255 => FrameworkSensorChip::Bma255,
        MotionSenseChip::Bmp280 => FrameworkSensorChip::Bmp280,
        MotionSenseChip::Opt3001 => FrameworkSensorChip::Opt3001,
        MotionSenseChip::Bh1730 => FrameworkSensorChip::Bh1730,
        MotionSenseChip::Gpio => FrameworkSensorChip::Gpio,
        MotionSenseChip::Lis2dh => FrameworkSensorChip::Lis2dh,
        MotionSenseChip::Lsm6dsm => FrameworkSensorChip::Lsm6dsm,
        MotionSenseChip::Lis2de => FrameworkSensorChip::Lis2de,
        MotionSenseChip::Lis2mdl => FrameworkSensorChip::Lis2mdl,
        MotionSenseChip::Lsm6ds3 => FrameworkSensorChip::Lsm6ds3,
        MotionSenseChip::Lsm6dso => FrameworkSensorChip::Lsm6dso,
        MotionSenseChip::Lng2dm => FrameworkSensorChip::Lng2dm,
        MotionSenseChip::Tcs3400 => FrameworkSensorChip::Tcs3400,
        MotionSenseChip::Lis2dw12 => FrameworkSensorChip::Lis2dw12,
        MotionSenseChip::Lis2dwl => FrameworkSensorChip::Lis2dwl,
        MotionSenseChip::Lis2ds => FrameworkSensorChip::Lis2ds,
        MotionSenseChip::Bmi260 => FrameworkSensorChip::Bmi260,
        MotionSenseChip::Icm426xx => FrameworkSensorChip::Icm426xx,
        MotionSenseChip::Icm42607 => FrameworkSensorChip::Icm42607,
        MotionSenseChip::Bma422 => FrameworkSensorChip::Bma422,
        MotionSenseChip::Bmi323 => FrameworkSensorChip::Bmi323,
        MotionSenseChip::Bmi220 => FrameworkSensorChip::Bmi220,
        MotionSenseChip::Cm32183 => FrameworkSensorChip::Cm32183,
        MotionSenseChip::Veml3328 => FrameworkSensorChip::Veml3328,
    }
}
