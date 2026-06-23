use framework_lib::chromium_ec::commands::EcRequestGetPdPortState;
use framework_lib::chromium_ec::{CrosEc, EcRequestRaw};

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
