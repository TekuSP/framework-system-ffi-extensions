use framework_lib::chromium_ec::commands::{DeckStateMode, EcRequestGetUptimeInfo, EcRequestSetTabletMode};
use framework_lib::chromium_ec::{CrosEc, EcError, EcRequestRaw};

use crate::{FrameworkDeckStateMode, FrameworkTabletModeOverride};

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
