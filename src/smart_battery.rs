//! Smart Battery (SBS) readback.
//!
//! Wraps upstream `framework_lib::smart_battery`, which talks to the battery
//! over I2C passthrough. A full collection is many round trips, so this is an
//! on-demand call rather than part of the polled power snapshot.

use framework_lib::chromium_ec::{CrosEc, EcError};
use framework_lib::smart_battery::{BatteryData, SmartBattery};

use crate::{FrameworkByteBuffer, FrameworkSmartBatteryData};

/// Reads the full Smart Battery data set.
///
/// `unseal_key` unlocks the manufacturer-access registers that back
/// `state_of_health`, the safety/PF status words and the lifetime blocks.
/// Pass `None` to read only the sealed-mode subset.
pub(crate) fn collect(ec: &CrosEc, unseal_key: Option<u32>) -> Result<BatteryData, EcError> {
    SmartBattery::new().collect_data(ec, unseal_key)
}

/// Runs the SHA-1 HMAC challenge/response battery authentication.
///
/// Returns whether the battery's response matched the challenge computed from
/// `auth_key`. A false result means the battery failed authentication, which is
/// distinct from an EC transport error.
pub(crate) fn authenticate(ec: &CrosEc, auth_key: &[u8; 16]) -> Result<bool, EcError> {
    SmartBattery::new().authenticate_battery(ec, auth_key)
}

/// Decodes the packed SBS ManufactureDate word into year/month/day.
///
/// Layout matches upstream `display_battery_data`: day in bits 0-4, month in
/// bits 5-8, year offset from 1980 in bits 9-15.
pub(crate) fn decode_manufacture_date(raw: u16) -> (u16, u8, u8) {
    let day = (raw & 0x1F) as u8;
    let month = ((raw >> 5) & 0x0F) as u8;
    let year = (raw >> 9) + 1980;
    (year, month, day)
}

pub(crate) fn into_ffi(data: BatteryData) -> FrameworkSmartBatteryData {
    let (manufacture_year, manufacture_month, manufacture_day) =
        decode_manufacture_date(data.manufacture_date);
    // The unsealed registers stay empty when no unseal key was supplied or the
    // battery rejected it, so state_of_health doubles as the "did we get in" tell.
    let unsealed = !data.state_of_health.is_empty();

    FrameworkSmartBatteryData {
        mode: data.mode,
        serial_number: data.serial_num,
        manufacture_date_raw: data.manufacture_date,
        manufacture_year,
        manufacture_month,
        manufacture_day,
        temperature_decikelvin: data.temperature,
        voltage_mv: data.voltage,
        cell_voltage_1_mv: data.cell_voltage1,
        cell_voltage_2_mv: data.cell_voltage2,
        cell_voltage_3_mv: data.cell_voltage3,
        cell_voltage_4_mv: data.cell_voltage4,
        cycle_count: data.cycle_count,
        current_ma: data.current as i16,
        avg_current_ma: data.avg_current as i16,
        rel_state_of_charge: data.rel_state_of_charge,
        abs_state_of_charge: data.abs_state_of_charge,
        remaining_capacity: data.remaining_capacity,
        full_charge_capacity: data.full_charge_capacity,
        charging_current_ma: data.charging_current as i16,
        charging_voltage_mv: data.charging_voltage,
        battery_status: data.battery_status,
        design_capacity: data.design_capacity,
        design_voltage_mv: data.design_voltage,
        operation_status: data.operation_status,
        safety_alert: data.safety_alert,
        safety_status: data.safety_status,
        pf_alert: data.pf_alert,
        pf_status: data.pf_status,
        unsealed: u8::from(unsealed),
        reserved: [0; 3],
        device_name: FrameworkByteBuffer::from_vec(data.device_name.into_bytes()),
        manufacturer_name: FrameworkByteBuffer::from_vec(data.manufacturer_name.into_bytes()),
        device_chemistry: FrameworkByteBuffer::from_vec(data.device_chemistry.into_bytes()),
        state_of_health: FrameworkByteBuffer::from_vec(data.state_of_health),
        firmware_version: FrameworkByteBuffer::from_vec(data.firmware_version),
        lifetime_1: FrameworkByteBuffer::from_vec(data.lifetime1),
        lifetime_2: FrameworkByteBuffer::from_vec(data.lifetime2),
        lifetime_3: FrameworkByteBuffer::from_vec(data.lifetime3),
        lifetime_4: FrameworkByteBuffer::from_vec(data.lifetime4),
        lifetime_5: FrameworkByteBuffer::from_vec(data.lifetime5),
    }
}

pub(crate) fn default_ffi() -> FrameworkSmartBatteryData {
    FrameworkSmartBatteryData {
        mode: 0,
        serial_number: 0,
        manufacture_date_raw: 0,
        manufacture_year: 0,
        manufacture_month: 0,
        manufacture_day: 0,
        temperature_decikelvin: 0,
        voltage_mv: 0,
        cell_voltage_1_mv: 0,
        cell_voltage_2_mv: 0,
        cell_voltage_3_mv: 0,
        cell_voltage_4_mv: 0,
        cycle_count: 0,
        current_ma: 0,
        avg_current_ma: 0,
        rel_state_of_charge: 0,
        abs_state_of_charge: 0,
        remaining_capacity: 0,
        full_charge_capacity: 0,
        charging_current_ma: 0,
        charging_voltage_mv: 0,
        battery_status: 0,
        design_capacity: 0,
        design_voltage_mv: 0,
        operation_status: 0,
        safety_alert: 0,
        safety_status: 0,
        pf_alert: 0,
        pf_status: 0,
        unsealed: 0,
        reserved: [0; 3],
        device_name: FrameworkByteBuffer::default(),
        manufacturer_name: FrameworkByteBuffer::default(),
        device_chemistry: FrameworkByteBuffer::default(),
        state_of_health: FrameworkByteBuffer::default(),
        firmware_version: FrameworkByteBuffer::default(),
        lifetime_1: FrameworkByteBuffer::default(),
        lifetime_2: FrameworkByteBuffer::default(),
        lifetime_3: FrameworkByteBuffer::default(),
        lifetime_4: FrameworkByteBuffer::default(),
        lifetime_5: FrameworkByteBuffer::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manufacture_date_decodes_upstream_layout() {
        // 2024-03-15 => ((2024-1980) << 9) | (3 << 5) | 15
        let raw = (44u16 << 9) | (3u16 << 5) | 15u16;
        assert_eq!(decode_manufacture_date(raw), (2024, 3, 15));
    }

    #[test]
    fn manufacture_date_zero_is_the_epoch_year() {
        assert_eq!(decode_manufacture_date(0), (1980, 0, 0));
    }
}
