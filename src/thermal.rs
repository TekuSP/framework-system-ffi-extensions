use framework_lib::chromium_ec::commands::EcFeatureCode;
use framework_lib::chromium_ec::{CrosEc, CrosEcDriver};
use framework_lib::smbios;
use framework_lib::smbios::PlatformFamily;

use crate::{
    FrameworkBatterySnapshot, FrameworkBatteryState, FrameworkByteBuffer, FrameworkFanCapabilities,
    FrameworkFanFeaturesState, FrameworkFanName, FrameworkFanReading, FrameworkFanState,
    FrameworkPowerSnapshot, FrameworkPowerSourceState, FrameworkStatus,
    FrameworkTemperatureReading, FrameworkTemperatureState, FrameworkThermalSnapshot,
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
        reserved: 0,
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
    let mut temperatures = [default_temperature_reading(); THERMAL_SENSOR_COUNT];
    for (index, reading) in snapshot.temperatures.iter().enumerate() {
        temperatures[index] = FrameworkTemperatureReading {
            state: reading.status.into(),
            celsius: reading.celsius,
            reserved: 0,
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
