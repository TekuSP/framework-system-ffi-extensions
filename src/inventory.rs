mod builder;
mod capabilities;
mod conversions;
mod defaults;
mod detect;
mod internals;
mod query;
mod usb_slots;

pub(crate) use builder::build_module_inventory;
pub(crate) use conversions::{fingerprint_led_level, fp_led_brightness_level};
pub(crate) use defaults::{
    default_expansion_bay_status, default_expansion_bay_status_result,
    default_feature_flags_result, default_fingerprint_led_result,
    default_keyboard_backlight_result, default_module_inventory_result,
    expansion_bay_status_result, feature_flags_result, fingerprint_led_result,
    keyboard_backlight_result, module_inventory_result,
};
pub(crate) use query::{expansion_bay_status, feature_flags};
