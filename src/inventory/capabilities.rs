//! Static per-platform USB-C expansion-slot capability tables.
//!
//! These are board-design facts (data lane, DisplayPort version, charging) that the EC does **not** report; they
//! come from Framework's published "Expansion Card Slot Functionality" matrices. Keyed on the specific
//! `FrameworkPlatform` (CPU generation), because capabilities differ across CPU options within a family.
//!
//! Slot index is zero-based and matches the EC PD-port index (slot 0 == documented "Port 1").

use crate::{
    FrameworkDisplayPortCapability as Dp, FrameworkExpansionBayVendor, FrameworkPlatform,
    FrameworkUsbCDataLane as Lane, FrameworkUsbCPortCapability, FrameworkUsbCPortPosition as Pos,
};

const fn cap(
    data_lane: Lane,
    displayport: Dp,
    supports_pd: bool,
    max_charge_watts: u16,
    usb_a_high_power: bool,
) -> FrameworkUsbCPortCapability {
    FrameworkUsbCPortCapability {
        known: 1,
        supports_pd: supports_pd as u8,
        usb_a_high_power: usb_a_high_power as u8,
        // Placeholder; slot_capability() sets the real position from the platform + index.
        position: Pos::Unknown,
        data_lane,
        displayport,
        max_charge_watts,
    }
}

/// Returned for platforms/slots not covered by a table (`known == 0`); the managed layer treats it as "no
/// documented capability" and falls back to the live PD state alone.
pub(super) const fn unknown_capability() -> FrameworkUsbCPortCapability {
    FrameworkUsbCPortCapability {
        known: 0,
        supports_pd: 0,
        usb_a_high_power: 0,
        position: Pos::Unknown,
        data_lane: Lane::Unknown,
        displayport: Dp::None,
        max_charge_watts: 0,
    }
}

/// Physical position of an EC PD port, mirroring upstream framework-system `power.rs get_and_print_pd_info`:
/// 0 Right Back, 1 Right Middle/Front, 2 Left Middle/Front, 3 Left Back — "Middle" on Framework 16, "Front" on the
/// other laptops. Only the documented 12/13/16 laptops get a mapping; anything else is `Unknown`.
fn port_position(platform: FrameworkPlatform, index: usize) -> Pos {
    let fl16 = matches!(
        platform,
        FrameworkPlatform::Framework16Amd7080 | FrameworkPlatform::Framework16AmdAi300
    );
    let is_laptop = fl16
        || matches!(
            platform,
            FrameworkPlatform::Framework12IntelGen13
                | FrameworkPlatform::Framework13Amd7080
                | FrameworkPlatform::Framework13AmdAi300
                | FrameworkPlatform::IntelGen11
                | FrameworkPlatform::IntelGen12
                | FrameworkPlatform::IntelGen13
                | FrameworkPlatform::IntelCoreUltra1
                | FrameworkPlatform::IntelCoreUltra3
        );
    if !is_laptop {
        return Pos::Unknown;
    }
    match index {
        0 => Pos::RightBack,
        1 => {
            if fl16 {
                Pos::RightMiddle
            } else {
                Pos::RightFront
            }
        }
        2 => {
            if fl16 {
                Pos::LeftMiddle
            } else {
                Pos::LeftFront
            }
        }
        3 => Pos::LeftBack,
        _ => Pos::Unknown,
    }
}

/// Look up the static capability for a slot on a given platform. `max_charge_watts == 0` with `supports_pd == 1`
/// means "charges, but the source matrix did not document a wattage".
pub(super) fn slot_capability(
    platform: FrameworkPlatform,
    slot_index: usize,
) -> FrameworkUsbCPortCapability {
    // Each table is in documented port order (Port 1 == index 0).
    let table: &[FrameworkUsbCPortCapability] = match platform {
        // Framework Laptop 16 — AMD Ryzen AI 300. FOUR EC PD ports in upstream order (power.rs): 0 Right Back,
        // 1 Right Middle, 2 Left Middle, 3 Left Back. The six physical bays mux onto these four controllers; the
        // two 900 mA "Front" bays are not PD ports, and the graphics module adds a 5th PD port (handled via the bay).
        FrameworkPlatform::Framework16AmdAi300 => &[
            cap(Lane::Usb4, Dp::Dp21Uhbr10, true, 240, true), // 0 Right Back
            cap(Lane::Usb32, Dp::Dp14Hbr3, true, 240, false), // 1 Right Middle
            cap(Lane::Usb32, Dp::Dp21Uhbr10, true, 240, false), // 2 Left Middle
            cap(Lane::Usb4, Dp::Dp21Uhbr10, true, 240, true), // 3 Left Back
        ],
        // Framework Laptop 16 — AMD Ryzen 7040 (four PD ports; DP versions not legible in the diagram → Supported).
        FrameworkPlatform::Framework16Amd7080 => &[
            cap(Lane::Usb4, Dp::Supported, true, 240, true), // 0 Right Back
            cap(Lane::Usb32, Dp::None, true, 240, false),    // 1 Right Middle
            cap(Lane::Usb32, Dp::Supported, true, 240, false), // 2 Left Middle
            cap(Lane::Usb4, Dp::Supported, true, 240, true), // 3 Left Back
        ],
        // Framework Laptop 13 — AMD Ryzen AI 300 (also FW13 Pro AI 300). PD 3.1 SPR, 20V/5A = 100 W.
        FrameworkPlatform::Framework13AmdAi300 => &[
            cap(Lane::Usb4, Dp::Dp20Uhbr20, true, 100, true),
            cap(Lane::Usb32Gen2x1, Dp::Dp20Uhbr10, true, 100, false),
            cap(Lane::Usb4, Dp::Dp20Uhbr20, true, 100, true),
            cap(Lane::Usb32Gen2x1, Dp::Dp14Hbr3, true, 100, false),
        ],
        // Framework Laptop 13 — AMD Ryzen 7040. PD 3.0, 100 W; port 2 has no DisplayPort.
        FrameworkPlatform::Framework13Amd7080 => &[
            cap(Lane::Usb4, Dp::Dp14Hbr3, true, 100, true),
            cap(Lane::Usb32Gen2x1, Dp::None, true, 100, false),
            cap(Lane::Usb4, Dp::Dp14Hbr3, true, 100, true),
            cap(Lane::Usb32Gen2x1, Dp::Dp14Hbr3, true, 100, false),
        ],
        // Framework Laptop 13 — Intel 11th/12th/13th Gen. Thunderbolt 4, DP 1.4, PD 3.0 100 W.
        FrameworkPlatform::IntelGen11
        | FrameworkPlatform::IntelGen12
        | FrameworkPlatform::IntelGen13 => &[
            cap(Lane::Thunderbolt4, Dp::Dp14Hbr3, true, 100, false),
            cap(Lane::Thunderbolt4, Dp::Dp14Hbr3, true, 100, false),
            cap(Lane::Thunderbolt4, Dp::Dp14Hbr3, true, 100, false),
            cap(Lane::Thunderbolt4, Dp::Dp14Hbr3, true, 100, false),
        ],
        // Framework Laptop 13 — Intel Core Ultra Series 1. Thunderbolt 4, DP 2.0, PD 3.0 100 W.
        FrameworkPlatform::IntelCoreUltra1 => &[
            cap(Lane::Thunderbolt4, Dp::Dp20, true, 100, false),
            cap(Lane::Thunderbolt4, Dp::Dp20, true, 100, false),
            cap(Lane::Thunderbolt4, Dp::Dp20, true, 100, false),
            cap(Lane::Thunderbolt4, Dp::Dp20, true, 100, false),
        ],
        // Framework Laptop 13 Pro — Intel Core Ultra Series 3. Thunderbolt 4, DP 2.1 UHBR20, up to 140 W.
        FrameworkPlatform::IntelCoreUltra3 => &[
            cap(Lane::Thunderbolt4, Dp::Dp21Uhbr20, true, 140, false),
            cap(Lane::Thunderbolt4, Dp::Dp21Uhbr20, true, 140, false),
            cap(Lane::Thunderbolt4, Dp::Dp21Uhbr20, true, 140, false),
            cap(Lane::Thunderbolt4, Dp::Dp21Uhbr20, true, 140, false),
        ],
        // Framework Laptop 12 — 13th Gen Intel (slots 1-2 Gen2x1, slots 3-4 Gen2x2; all DP 1.4 HBR3, all charge).
        // Charging is 64 W officially (≈74 W observed maximum).
        FrameworkPlatform::Framework12IntelGen13 => &[
            cap(Lane::Usb32Gen2x1, Dp::Dp14Hbr3, true, 64, false),
            cap(Lane::Usb32Gen2x1, Dp::Dp14Hbr3, true, 64, false),
            cap(Lane::Usb32Gen2x2, Dp::Dp14Hbr3, true, 64, false),
            cap(Lane::Usb32Gen2x2, Dp::Dp14Hbr3, true, 64, false),
        ],
        // Framework Desktop — AMD Ryzen AI Max 300: two front expansion slots, USB 3.2 only (no DP, no charging).
        FrameworkPlatform::FrameworkDesktopAmdAiMax300 => &[
            cap(Lane::Usb32, Dp::None, false, 0, false),
            cap(Lane::Usb32, Dp::None, false, 0, false),
        ],
        // GenericFramework / UnknownSystem → no documented matrix.
        _ => return unknown_capability(),
    };

    let mut capability = table
        .get(slot_index)
        .copied()
        .unwrap_or_else(unknown_capability);
    capability.position = port_position(platform, slot_index);
    capability
}

/// Static capability of a Framework 16 graphics-module USB-C port, keyed on the expansion-bay vendor. The GPU
/// modules each expose a single rear USB-C port; non-GPU bay contents (fans, SSD holder, …) have none.
pub(super) fn gpu_module_capability(
    vendor: FrameworkExpansionBayVendor,
) -> FrameworkUsbCPortCapability {
    let mut capability = match vendor {
        // NVIDIA GeForce RTX graphics module: USB 2.0 / DisplayPort 2.1 / up to 240 W charging.
        FrameworkExpansionBayVendor::NvidiaGpu => cap(Lane::Usb2, Dp::Dp21, true, 240, false),
        // Radeon RX graphics module: USB 2.0 / DisplayPort 2.1, no charging.
        FrameworkExpansionBayVendor::AmdGpu => cap(Lane::Usb2, Dp::Dp21, false, 0, false),
        _ => return unknown_capability(),
    };
    capability.position = Pos::GraphicsModule;
    capability
}

#[cfg(test)]
mod tests {
    use super::*;

    const FW16: FrameworkPlatform = FrameworkPlatform::Framework16AmdAi300;

    #[test]
    fn framework16_has_four_documented_mainboard_pd_ports() {
        for index in 0..4 {
            assert_eq!(
                slot_capability(FW16, index).known,
                1,
                "mainboard PD port {index} should be documented"
            );
        }
        assert_eq!(
            slot_capability(FW16, 4).known,
            0,
            "there is no 5th mainboard PD port; the graphics module is a separate bay port"
        );
    }

    #[test]
    fn framework16_positions_match_upstream_order() {
        assert_eq!(slot_capability(FW16, 0).position, Pos::RightBack);
        assert_eq!(slot_capability(FW16, 1).position, Pos::RightMiddle);
        assert_eq!(slot_capability(FW16, 2).position, Pos::LeftMiddle);
        assert_eq!(slot_capability(FW16, 3).position, Pos::LeftBack);
    }

    #[test]
    fn framework16_ports_two_and_three_are_left() {
        // Upstream check_ac: ports 0/1 = right, 2/3 = left.
        let left = |i| {
            matches!(
                slot_capability(FW16, i).position,
                Pos::LeftMiddle | Pos::LeftFront | Pos::LeftBack
            )
        };
        assert!(!left(0));
        assert!(!left(1));
        assert!(left(2));
        assert!(left(3));
    }

    #[test]
    fn framework13_uses_front_positions_not_middle() {
        let position = |i| slot_capability(FrameworkPlatform::Framework13AmdAi300, i).position;
        assert_eq!(position(1), Pos::RightFront);
        assert_eq!(position(2), Pos::LeftFront);
    }

    #[test]
    fn nvidia_graphics_module_is_a_charging_pd_port() {
        // The 5th PD port (4 mainboard + GPU = 5).
        let gpu = gpu_module_capability(FrameworkExpansionBayVendor::NvidiaGpu);
        assert_eq!(gpu.position, Pos::GraphicsModule);
        assert_eq!(gpu.supports_pd, 1, "the NVIDIA module charges");
        assert_eq!(gpu.max_charge_watts, 240);
    }
}
