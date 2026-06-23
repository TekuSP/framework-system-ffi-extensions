use framework_lib::chromium_ec::commands::{
    DeckStateMode, EcRequestGetUptimeInfo, EcRequestReadBoardId, EcRequestSetTabletMode,
    MotionSenseChip, MotionSenseInfo, MotionSenseLocation, MotionSenseType,
};
use framework_lib::chromium_ec::{CrosEc, CrosEcDriver, EcError, EcRequestRaw};

use crate::{
    FrameworkAccelDataResult, FrameworkDeckStateMode, FrameworkSensorCategory,
    FrameworkSensorChip, FrameworkSensorDescriptor, FrameworkSensorLocation, FrameworkSensorType,
    FrameworkStatus, FrameworkTabletModeOverride,
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

pub(crate) fn set_tablet_mode(ec: &CrosEc, mode: FrameworkTabletModeOverride) -> Result<(), EcError> {
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
    let lid_angle_degrees = if lid_angle_raw == LID_ANGLE_UNRELIABLE { -1i16 } else { lid_angle_raw as i16 };

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
        FrameworkSensorType::Accel | FrameworkSensorType::Gyro | FrameworkSensorType::Mag
            => FrameworkSensorCategory::Motion,
        FrameworkSensorType::Light | FrameworkSensorType::LightRgb
        | FrameworkSensorType::Prox | FrameworkSensorType::Baro
            => FrameworkSensorCategory::Environmental,
        FrameworkSensorType::Activity | FrameworkSensorType::Sync
            => FrameworkSensorCategory::Other,
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
