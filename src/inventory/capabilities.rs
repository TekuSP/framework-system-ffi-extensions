//! Static per-platform USB-C expansion-slot capability tables.
//!
//! These are board-design facts (data lane, DisplayPort version, charging) that the EC does **not** report; they
//! come from Framework's published "Expansion Card Slot Functionality" matrices. Keyed on the specific
//! `FrameworkPlatform` (CPU generation), because capabilities differ across CPU options within a family.
//!
//! Slot index is zero-based and matches the EC PD-port index (slot 0 == documented "Port 1").

use crate::{
    FrameworkDisplayPortCapability as Dp, FrameworkExpansionBayVendor, FrameworkPlatform,
    FrameworkUsbCDataLane as Lane, FrameworkUsbCPortCapability,
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
        reserved_0: 0,
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
        reserved_0: 0,
        data_lane: Lane::Unknown,
        displayport: Dp::None,
        max_charge_watts: 0,
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
        // Framework Laptop 16 — AMD Ryzen AI 300 (slots 3 & 6 are 900 mA, no PD, no DP).
        FrameworkPlatform::Framework16AmdAi300 => &[
            cap(Lane::Usb4, Dp::Dp21Uhbr10, true, 240, true),
            cap(Lane::Usb32, Dp::Dp21Uhbr10, true, 240, false),
            cap(Lane::Usb32, Dp::None, false, 0, false),
            cap(Lane::Usb4, Dp::Dp21Uhbr10, true, 240, true),
            cap(Lane::Usb32, Dp::Dp14Hbr3, true, 240, false),
            cap(Lane::Usb32, Dp::None, false, 0, false),
        ],
        // Framework Laptop 16 — AMD Ryzen 7040 (DP versions not legible in the diagram → Supported).
        FrameworkPlatform::Framework16Amd7080 => &[
            cap(Lane::Usb4, Dp::Supported, true, 240, true),
            cap(Lane::Usb32, Dp::Supported, true, 240, false),
            cap(Lane::Usb32, Dp::None, false, 0, false),
            cap(Lane::Usb4, Dp::Supported, true, 240, true),
            cap(Lane::Usb32, Dp::None, true, 240, false),
            cap(Lane::Usb32, Dp::None, false, 0, false),
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

    table
        .get(slot_index)
        .copied()
        .unwrap_or_else(unknown_capability)
}

/// Static capability of a Framework 16 graphics-module USB-C port, keyed on the expansion-bay vendor. The GPU
/// modules each expose a single rear USB-C port; non-GPU bay contents (fans, SSD holder, …) have none.
pub(super) fn gpu_module_capability(
    vendor: FrameworkExpansionBayVendor,
) -> FrameworkUsbCPortCapability {
    match vendor {
        // NVIDIA GeForce RTX graphics module: USB 2.0 / DisplayPort 2.1 / up to 240 W charging.
        FrameworkExpansionBayVendor::NvidiaGpu => cap(Lane::Usb2, Dp::Dp21, true, 240, false),
        // Radeon RX graphics module: USB 2.0 / DisplayPort 2.1, no charging.
        FrameworkExpansionBayVendor::AmdGpu => cap(Lane::Usb2, Dp::Dp21, false, 0, false),
        _ => unknown_capability(),
    }
}
