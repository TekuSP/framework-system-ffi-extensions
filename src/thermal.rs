use framework_lib::chromium_ec::commands::{EcFeatureCode, EcRequestTempSensorGetInfo};
use framework_lib::chromium_ec::{CrosEc, CrosEcDriver, EcRequestRaw};
use framework_lib::power;
use framework_lib::smbios;
use framework_lib::smbios::Platform;
use framework_lib::smbios::PlatformFamily;

use crate::{
    FrameworkBatterySnapshot, FrameworkBatteryState, FrameworkByteBuffer, FrameworkFanCapabilities,
    FrameworkFanFeaturesState, FrameworkFanName, FrameworkFanReading, FrameworkFanState,
    FrameworkPowerSnapshot, FrameworkPowerSourceState, FrameworkSensorName, FrameworkStatus,
    FrameworkTemperatureReading, FrameworkTemperatureState, FrameworkThermalSnapshot,
    FrameworkThermalThresholdFlag, FrameworkThermalThresholds,
};

const THERMAL_SENSOR_COUNT: usize = 8;
const FAN_SLOT_COUNT: usize = 4;
const EC_MEMMAP_TEMP_SENSOR: u16 = 0x00;
const EC_MEMMAP_FAN: u16 = 0x10;
const EC_FAN_SPEED_STALLED_DEPRECATED: u16 = 0xFFFE;
const EC_FAN_SPEED_NOT_PRESENT: u16 = 0xFFFF;

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

pub(crate) fn power_source_state(
    ac_present: bool,
    battery_present: bool,
) -> FrameworkPowerSourceState {
    match (ac_present, battery_present) {
        (false, false) => FrameworkPowerSourceState::None,
        (true, false) => FrameworkPowerSourceState::AcOnly,
        (false, true) => FrameworkPowerSourceState::BatteryOnly,
        (true, true) => FrameworkPowerSourceState::AcAndBattery,
    }
}

pub(crate) fn battery_state(
    level_critical: bool,
    discharging: bool,
    charging: bool,
) -> FrameworkBatteryState {
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
        name: FrameworkSensorName::Unknown,
    }
}

/// Maps a temperature sensor slot to its platform role name. This is the return-based mirror of the
/// per-platform sensor labels that `framework_lib::power::print_thermal` only prints to stdout; slots
/// past the labelled set (and the generic fallback platform) resolve to `Generic`.
fn sensor_name(
    index: usize,
    platform: Option<Platform>,
    family: Option<PlatformFamily>,
) -> FrameworkSensorName {
    use FrameworkSensorName as N;

    match platform {
        Some(Platform::IntelGen11 | Platform::IntelGen12 | Platform::IntelGen13) => match index {
            0 => N::F75303Local,
            1 => N::F75303Cpu,
            2 => N::F75303Ddr,
            3 => N::Battery,
            4 => N::Peci,
            5 if matches!(platform, Some(Platform::IntelGen12 | Platform::IntelGen13)) => {
                N::F57397VccGt
            }
            _ => N::Generic,
        },
        Some(Platform::IntelCoreUltra1) => match index {
            0 => N::F75303Local,
            1 => N::F75303Cpu,
            2 => N::Battery,
            3 => N::F75303Ddr,
            4 => N::Peci,
            _ => N::Generic,
        },
        Some(Platform::Framework12IntelGen13) => match index {
            0 => N::F75303Cpu,
            1 => N::F75303Skin,
            2 => N::F75303Local,
            3 => N::Battery,
            4 => N::Peci,
            5 => N::ChargerIc,
            _ => N::Generic,
        },
        Some(
            Platform::Framework13Amd7080
            | Platform::Framework13AmdAi300
            | Platform::Framework16Amd7080
            | Platform::Framework16AmdAi300,
        ) => {
            // Framework 16 reports four extra dGPU sensors after the shared APU set.
            if family == Some(PlatformFamily::Framework16) {
                match index {
                    0 => N::F75303Local,
                    1 => N::F75303Cpu,
                    2 => N::F75303Ddr,
                    3 => N::Apu,
                    4 => N::DgpuVr,
                    5 => N::DgpuVram,
                    6 => N::DgpuAmb,
                    7 => N::DgpuTemp,
                    _ => N::Generic,
                }
            } else {
                match index {
                    0 => N::F75303Local,
                    1 => N::F75303Cpu,
                    2 => N::F75303Ddr,
                    3 => N::Apu,
                    _ => N::Generic,
                }
            }
        }
        Some(Platform::FrameworkDesktopAmdAiMax300) => match index {
            0 => N::F75303Apu,
            1 => N::F75303Ddr,
            2 => N::F75303Amb,
            3 => N::Apu,
            4 => N::Virtual,
            _ => N::Generic,
        },
        Some(_) => N::Generic,
        None => N::Unknown,
    }
}

fn fan_name(fan_index: usize, family: Option<PlatformFamily>) -> FrameworkFanName {
    match (fan_index, family) {
        (0, Some(PlatformFamily::Framework12)) => FrameworkFanName::ApuFan,
        (0, Some(PlatformFamily::Framework13)) => FrameworkFanName::ApuFan,
        (0, Some(PlatformFamily::Framework16)) => FrameworkFanName::LeftFan,
        (1, Some(PlatformFamily::Framework16)) => FrameworkFanName::RightFan,
        (0, Some(PlatformFamily::FrameworkDesktop)) => FrameworkFanName::ApuFan,
        (1, Some(PlatformFamily::FrameworkDesktop)) => FrameworkFanName::FrontFan,
        (2, Some(PlatformFamily::FrameworkDesktop)) => FrameworkFanName::ThirdFan,
        (_, Some(_)) => FrameworkFanName::Generic,
        (_, None) => FrameworkFanName::Unknown,
    }
}

fn default_fan_reading() -> FrameworkFanReading {
    FrameworkFanReading {
        state: FrameworkFanState::NotPresent,
        rpm: 0,
        name: FrameworkFanName::Unknown,
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

pub(crate) fn default_power_snapshot() -> FrameworkPowerSnapshot {
    FrameworkPowerSnapshot {
        power_source_state: FrameworkPowerSourceState::None,
        battery_count: 0,
        reserved: [0; 2],
        battery_0: default_battery_snapshot(),
    }
}

pub(crate) fn default_fan_capabilities() -> FrameworkFanCapabilities {
    FrameworkFanCapabilities {
        fan_count: 0,
        features: FrameworkFanFeaturesState::None,
        reserved: [0; 2],
    }
}

pub(crate) fn default_thermal_snapshot() -> FrameworkThermalSnapshot {
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

pub(crate) fn build_fan_capabilities(
    ec: &CrosEc,
) -> Result<FrameworkFanCapabilities, FrameworkStatus> {
    let fan_control = crate::feature_enabled(ec, EcFeatureCode::PwmFan)?;
    let thermal = crate::feature_enabled(ec, EcFeatureCode::Thermal)?;
    let fan_count = thermal_snapshot(ec)
        .map(|snapshot| snapshot.fan_count)
        .unwrap_or(0);

    Ok(FrameworkFanCapabilities {
        fan_count,
        features: fan_features_state(fan_control, thermal),
        reserved: [0; 2],
    })
}

pub(crate) fn build_thermal_snapshot(ec: &CrosEc) -> Option<FrameworkThermalSnapshot> {
    let snapshot = thermal_snapshot(ec)?;
    let family = smbios::get_family();
    let platform = smbios::get_platform();
    let mut temperatures = [default_temperature_reading(); THERMAL_SENSOR_COUNT];
    for (index, reading) in snapshot.temperatures.iter().enumerate() {
        temperatures[index] = FrameworkTemperatureReading {
            state: reading.status.into(),
            celsius: reading.celsius,
            name: sensor_name(index, platform, family),
        };
    }

    Some(FrameworkThermalSnapshot {
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
            name: fan_name(0, family),
        },
        fan_1: FrameworkFanReading {
            state: fan_state(snapshot.fan_present[1], snapshot.fan_stalled[1]),
            rpm: snapshot.fan_rpms[1],
            name: fan_name(1, family),
        },
        fan_2: FrameworkFanReading {
            state: fan_state(snapshot.fan_present[2], snapshot.fan_stalled[2]),
            rpm: snapshot.fan_rpms[2],
            name: fan_name(2, family),
        },
        fan_3: FrameworkFanReading {
            state: fan_state(snapshot.fan_present[3], snapshot.fan_stalled[3]),
            rpm: snapshot.fan_rpms[3],
            name: fan_name(3, family),
        },
    })
}

/// Kelvin offset upstream uses when converting EC thermal thresholds to Celsius.
const KELVIN_OFFSET: i32 = 273;

fn threshold_celsius(kelvin: u32) -> i32 {
    kelvin as i32 - KELVIN_OFFSET
}

/// Reads the EC thermal configuration for one sensor.
///
/// Upstream stores thresholds in Kelvin with zero meaning "disabled"; the ABI
/// reports Celsius plus an `enabled_mask` so a disabled threshold is never
/// confused with a real sub-zero value.
pub(crate) fn get_thermal_thresholds(
    ec: &CrosEc,
    sensor_index: u32,
) -> Result<FrameworkThermalThresholds, framework_lib::chromium_ec::EcError> {
    let cfg = ec.get_thermal_threshold(sensor_index)?;
    // Copy out of the packed struct before taking references to the arrays.
    let temp_host = { cfg.temp_host };
    let temp_host_release = { cfg.temp_host_release };
    let temp_fan_off = { cfg.temp_fan_off };
    let temp_fan_max = { cfg.temp_fan_max };

    let mut enabled_mask = 0u32;
    let mut mark = |flag: FrameworkThermalThresholdFlag, kelvin: u32| {
        if kelvin != 0 {
            enabled_mask |= flag as u32;
        }
    };
    mark(FrameworkThermalThresholdFlag::Warn, temp_host[0]);
    mark(FrameworkThermalThresholdFlag::High, temp_host[1]);
    mark(FrameworkThermalThresholdFlag::Halt, temp_host[2]);
    mark(
        FrameworkThermalThresholdFlag::WarnRelease,
        temp_host_release[0],
    );
    mark(
        FrameworkThermalThresholdFlag::HighRelease,
        temp_host_release[1],
    );
    mark(
        FrameworkThermalThresholdFlag::HaltRelease,
        temp_host_release[2],
    );
    mark(FrameworkThermalThresholdFlag::FanOff, temp_fan_off);
    mark(FrameworkThermalThresholdFlag::FanMax, temp_fan_max);

    Ok(FrameworkThermalThresholds {
        enabled_mask,
        warn_celsius: threshold_celsius(temp_host[0]),
        high_celsius: threshold_celsius(temp_host[1]),
        halt_celsius: threshold_celsius(temp_host[2]),
        warn_release_celsius: threshold_celsius(temp_host_release[0]),
        high_release_celsius: threshold_celsius(temp_host_release[1]),
        halt_release_celsius: threshold_celsius(temp_host_release[2]),
        fan_off_celsius: threshold_celsius(temp_fan_off),
        fan_max_celsius: threshold_celsius(temp_fan_max),
    })
}

/// Writes thermal thresholds for one sensor via upstream's read-modify-write
/// helper. Each value follows upstream's convention: negative keeps the current
/// threshold, zero disables it, positive is degrees Celsius.
pub(crate) fn set_thermal_thresholds(
    ec: &CrosEc,
    sensor_index: u32,
    values: &[i32; 5],
) -> Result<(), framework_lib::chromium_ec::EcError> {
    power::set_thermal_thresholds(ec, sensor_index, values)
}

pub(crate) struct ApThrottle {
    pub soft: bool,
    pub hard: bool,
}

pub(crate) fn get_ap_throttle(
    ec: &CrosEc,
) -> Result<ApThrottle, framework_lib::chromium_ec::EcError> {
    let res = ec.get_ap_throttle_status()?;
    Ok(ApThrottle {
        soft: { res.soft_ap_throttle } == 1,
        hard: { res.hard_ap_throttle } == 1,
    })
}

/// Battery cutoff (ship mode) state. `None` when the EC does not answer.
pub(crate) fn get_cutoff_status(ec: &CrosEc) -> Option<bool> {
    power::get_cutoff_status(ec)
}

/// Reads a temperature sensor's name straight from EC firmware.
pub(crate) struct TempSensorInfo {
    pub name: String,
    /// EC `EC_TEMP_SENSOR_TYPE_*` tag: 0 ignored, 1 CPU, 2 board, 3 case, 4 battery.
    pub sensor_type: u8,
}

/// Reads a temperature sensor's name and type in one host command.
///
/// Upstream's `CrosEc::get_temp_sensor_name` discards the `sensor_type` byte, so
/// send `EcRequestTempSensorGetInfo` directly to keep both halves of the response.
/// Name decoding matches upstream: UTF-8, trailing NULs trimmed.
pub(crate) fn get_temp_sensor_info(
    ec: &CrosEc,
    index: u8,
) -> Result<TempSensorInfo, framework_lib::chromium_ec::EcError> {
    let res = EcRequestTempSensorGetInfo { id: index }.send_command(ec)?;
    // Copy out of the packed struct before borrowing the array.
    let sensor_name = { res.sensor_name };
    let name = std::str::from_utf8(&sensor_name)
        .map_err(|utf8_err| {
            framework_lib::chromium_ec::EcError::DeviceError(format!(
                "Failed to decode sensor name: {:?}",
                utf8_err
            ))
        })?
        .trim_end_matches(char::from(0))
        .to_string();

    Ok(TempSensorInfo {
        name,
        sensor_type: { res.sensor_type },
    })
}

/// Maps an EC-reported sensor name onto the stable `FrameworkSensorName` enum.
///
/// Firmware spelling varies by platform (`F75303_CPU`, `f75303 cpu`, `dGPU VR`),
/// so compare on a lowercased, separator-stripped form. Unrecognised names stay
/// `Generic` — the raw string is still returned alongside for display.
pub(crate) fn map_sensor_name(name: &str) -> FrameworkSensorName {
    use FrameworkSensorName as N;

    let normalized: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    match normalized.as_str() {
        "f75303local" => N::F75303Local,
        "f75303cpu" => N::F75303Cpu,
        "f75303ddr" => N::F75303Ddr,
        "f75303skin" => N::F75303Skin,
        "f75303apu" => N::F75303Apu,
        "f75303amb" => N::F75303Amb,
        "f57397vccgt" => N::F57397VccGt,
        "battery" => N::Battery,
        "peci" => N::Peci,
        "chargeric" => N::ChargerIc,
        "apu" => N::Apu,
        "dgpuvr" => N::DgpuVr,
        "dgpuvram" => N::DgpuVram,
        "dgpuamb" => N::DgpuAmb,
        "dgputemp" | "dgpu" => N::DgpuTemp,
        "virtual" => N::Virtual,
        _ => N::Generic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmware_sensor_names_map_onto_the_stable_enum() {
        assert_eq!(
            map_sensor_name("F75303_Local"),
            FrameworkSensorName::F75303Local
        );
        assert_eq!(
            map_sensor_name("F75303_CPU"),
            FrameworkSensorName::F75303Cpu
        );
        assert_eq!(map_sensor_name("APU"), FrameworkSensorName::Apu);
        assert_eq!(map_sensor_name("dGPU VR"), FrameworkSensorName::DgpuVr);
        assert_eq!(map_sensor_name("Virtual"), FrameworkSensorName::Virtual);
    }

    #[test]
    fn unknown_firmware_names_fall_back_to_generic() {
        assert_eq!(
            map_sensor_name("Some_New_Sensor"),
            FrameworkSensorName::Generic
        );
        assert_eq!(map_sensor_name(""), FrameworkSensorName::Generic);
    }

    #[test]
    fn disabled_thresholds_read_back_as_the_kelvin_zero_offset() {
        // Upstream stores 0 K for "disabled"; the mask is what callers must check.
        assert_eq!(threshold_celsius(0), -273);
        assert_eq!(threshold_celsius(273 + 80), 80);
    }
}
