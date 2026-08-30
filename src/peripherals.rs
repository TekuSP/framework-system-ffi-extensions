//! Touchscreen and touchpad controls.
//!
//! These talk to HID devices directly rather than through the EC, so they take no
//! EC handle. Upstream returns `Option`/`HidError`; map both onto plain booleans
//! and let `lib.rs` turn a failure into a `FrameworkStatus`.

use framework_lib::touchpad::{self, ClickForce};
use framework_lib::touchscreen;

use crate::FrameworkClickForce;

/// Enables or disables touch input on the touchscreen.
///
/// Returns false when no supported touchscreen answered.
pub(crate) fn enable_touchscreen(enable: bool) -> bool {
    touchscreen::enable_touch(enable).is_some()
}

/// Reads the stylus battery level as a percentage.
///
/// `None` when no stylus is paired or the touchscreen does not report it.
pub(crate) fn stylus_battery_level() -> Option<u8> {
    touchscreen::get_battery_level()
}

/// Sets haptic feedback intensity on a haptic touchpad.
///
/// The firmware accepts SET_FEATURE but never answers GET_FEATURE, so this is
/// write-only — there is no matching read.
pub(crate) fn set_haptic_intensity(value: u8) -> bool {
    touchpad::set_haptic_intensity(value).is_ok()
}

pub(crate) fn into_click_force(force: FrameworkClickForce) -> ClickForce {
    match force {
        FrameworkClickForce::Low => ClickForce::Low,
        FrameworkClickForce::Medium => ClickForce::Medium,
        FrameworkClickForce::High => ClickForce::High,
    }
}

/// Sets the click force threshold on a haptic touchpad. Write-only, as above.
pub(crate) fn set_click_force(force: FrameworkClickForce) -> bool {
    touchpad::set_click_force(into_click_force(force)).is_ok()
}
