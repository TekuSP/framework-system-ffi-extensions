use framework_lib::chromium_ec::commands::EcRequestGetPdPortState;
use framework_lib::chromium_ec::{CrosEc, EcError, EcRequestRaw};
use framework_lib::power::{self, UsbChargingType, UsbPowerRoles};

use crate::{
    FrameworkEcPdPortState, FrameworkPdCcPolarity, FrameworkPdDataRole, FrameworkPdPowerRole,
    FrameworkPdTypeCState,
};

pub(crate) fn default_pd_port_state() -> FrameworkEcPdPortState {
    FrameworkEcPdPortState {
        c_state: FrameworkPdTypeCState::Nothing,
        power_role: FrameworkPdPowerRole::Unknown,
        data_role: FrameworkPdDataRole::Unknown,
        cc_polarity: FrameworkPdCcPolarity::Unknown,
        voltage_mv: 0,
        current_ma: 0,
        has_pd_contract: 0,
        vconn_active: 0,
        epr_active: 0,
        epr_support: 0,
        active_port: 0,
        alt_mode_flags: 0,
        reserved: [0; 2],
    }
}

pub(crate) fn query_pd_port_state(ec: &CrosEc, port: u8) -> FrameworkEcPdPortState {
    let response = match (EcRequestGetPdPortState { port }).send_command(ec) {
        Ok(r) => r,
        Err(_) => return default_pd_port_state(),
    };

    FrameworkEcPdPortState {
        c_state: c_state(response.c_state),
        power_role: power_role(response.power_role),
        data_role: data_role(response.data_role),
        cc_polarity: cc_polarity(response.cc_polarity),
        voltage_mv: response.voltage,
        current_ma: response.current,
        has_pd_contract: response.pd_state,
        vconn_active: response.vconn,
        epr_active: response.epr_active,
        epr_support: response.epr_support,
        active_port: response.active_port,
        alt_mode_flags: response.pd_alt_mode_status,
        reserved: [0; 2],
    }
}

/// Per-port charger negotiation state from `EC_CMD_USB_PD_POWER_INFO`.
///
/// This is a different surface from `query_pd_port_state`: that one reports the
/// Type-C link, this one reports what the attached charger offers.
pub(crate) struct PdPowerInfo {
    pub role: u8,
    pub charging_type: u8,
    pub dualrole: bool,
    pub voltage_max_mv: u16,
    pub voltage_now_mv: u16,
    pub current_max_ma: u16,
    pub current_lim_ma: u16,
    pub max_power_uw: u32,
}

/// Reads charger info for one port.
///
/// Upstream only exposes `get_pd_info`, which sweeps ports `0..ports` and returns a
/// vector; call it for a single port so the ABI stays one port per call.
pub(crate) fn query_pd_power_info(ec: &CrosEc, port: u8) -> Result<PdPowerInfo, EcError> {
    let mut infos = power::get_pd_info(ec, port.saturating_add(1));
    // get_pd_info returns one entry per port from 0..=port; take the requested one.
    let info = infos
        .pop()
        .unwrap_or_else(|| Err(EcError::DeviceError(format!("No PD port {}", port))))?;

    Ok(PdPowerInfo {
        role: match info.role {
            UsbPowerRoles::Disconnected => 0,
            UsbPowerRoles::Source => 1,
            UsbPowerRoles::Sink => 2,
            UsbPowerRoles::SinkNotCharging => 3,
        },
        charging_type: match info.charging_type {
            UsbChargingType::None => 0,
            UsbChargingType::PD => 1,
            UsbChargingType::TypeC => 2,
            UsbChargingType::Proprietary => 3,
            UsbChargingType::Bc12Dcp => 4,
            UsbChargingType::Bc12Cdp => 5,
            UsbChargingType::Bc12Sdp => 6,
            UsbChargingType::Other => 7,
            UsbChargingType::VBus => 8,
            UsbChargingType::Unknown => 9,
        },
        dualrole: info.dualrole,
        voltage_max_mv: info.meas.voltage_max,
        voltage_now_mv: info.meas.voltage_now,
        current_max_ma: info.meas.current_max,
        current_lim_ma: info.meas.current_lim,
        max_power_uw: info.max_power,
    })
}

/// Whether the battery is charging, and whether AC is attached.
pub(crate) fn is_charging(ec: &CrosEc) -> Result<(bool, bool), EcError> {
    power::is_charging(ec)
}

fn c_state(v: u8) -> FrameworkPdTypeCState {
    match v {
        0 => FrameworkPdTypeCState::Nothing,
        1 => FrameworkPdTypeCState::Sink,
        2 => FrameworkPdTypeCState::Source,
        3 => FrameworkPdTypeCState::Debug,
        4 => FrameworkPdTypeCState::Audio,
        5 => FrameworkPdTypeCState::PoweredAccessory,
        6 => FrameworkPdTypeCState::Unsupported,
        _ => FrameworkPdTypeCState::Invalid,
    }
}

fn power_role(v: u8) -> FrameworkPdPowerRole {
    match v {
        0 => FrameworkPdPowerRole::Sink,
        1 => FrameworkPdPowerRole::Source,
        _ => FrameworkPdPowerRole::Unknown,
    }
}

fn data_role(v: u8) -> FrameworkPdDataRole {
    match v {
        0 => FrameworkPdDataRole::Ufp,
        1 => FrameworkPdDataRole::Dfp,
        2 => FrameworkPdDataRole::Disconnected,
        _ => FrameworkPdDataRole::Unknown,
    }
}

fn cc_polarity(v: u8) -> FrameworkPdCcPolarity {
    match v {
        0 => FrameworkPdCcPolarity::Cc1,
        1 => FrameworkPdCcPolarity::Cc2,
        2 => FrameworkPdCcPolarity::Cc1Debug,
        3 => FrameworkPdCcPolarity::Cc2Debug,
        _ => FrameworkPdCcPolarity::Unknown,
    }
}
