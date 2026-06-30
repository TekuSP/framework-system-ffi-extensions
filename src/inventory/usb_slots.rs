use crate::*;

use super::conversions::{
    expansion_card_identity, expansion_card_type, module_descriptor, module_flag,
};
use super::detect::{
    detect_audio_cards_local, detect_ethernet_cards_local, detect_expansion_cards_local,
    detect_microsd_cards_local, detect_sd_cards_local, detect_ssd_cards_local,
    detect_usb_a_cards_local, push_detached_module,
};
use crate::pd;

pub(super) const MAX_USB_C_SLOT_COUNT: usize = 6;

fn make_expansion_card(
    base: FrameworkModuleDescriptor,
    pd_state: FrameworkEcPdPortState,
    card_type: FrameworkExpansionCardType,
    card_confidence: FrameworkModuleConfidence,
) -> FrameworkExpansionCardModuleDescriptor {
    FrameworkExpansionCardModuleDescriptor {
        identity: base.identity,
        bus: base.bus,
        slot_kind: base.slot_kind,
        confidence: base.confidence,
        present: base.present,
        reserved_0: base.reserved_0,
        slot_index: base.slot_index,
        flags: base.flags,
        vendor_id: base.vendor_id,
        product_id: base.product_id,
        board_id: base.board_id,
        pd: pd_state,
        card_type,
        card_confidence,
        // Placeholder; the platform-specific slot capability is applied once at the end of populate_usb_slots.
        capability: super::capabilities::unknown_capability(),
        reserved: 0,
    }
}

fn pd_flags(pd: &FrameworkEcPdPortState) -> u32 {
    let connected = !matches!(pd.c_state, FrameworkPdTypeCState::Nothing);
    let dp_alt_mode = pd.alt_mode_flags & 0x03 != 0;
    let active = pd.active_port != 0 || dp_alt_mode;

    let mut flags = 0u32;
    if connected {
        flags |= module_flag(FrameworkModuleFlag::Connected);
    }
    if pd.has_pd_contract != 0 {
        flags |= module_flag(FrameworkModuleFlag::HasPdContract);
    }
    if dp_alt_mode {
        flags |= module_flag(FrameworkModuleFlag::DisplayAltMode);
    }
    if active {
        flags |= module_flag(FrameworkModuleFlag::Active);
    }
    flags
}

pub(super) fn populate_usb_slots(
    handle: &FrameworkEcHandle,
    usb_c_slot_count: u8,
    platform: FrameworkPlatform,
    usb_slots: &mut [FrameworkExpansionCardModuleDescriptor; MAX_USB_C_SLOT_COUNT],
    detached: &mut [FrameworkModuleDescriptor; 4],
    detached_count: &mut u8,
) {
    let mut pd_states = [None::<FrameworkEcPdPortState>; MAX_USB_C_SLOT_COUNT];

    for index in 0..(usb_c_slot_count as usize) {
        let pd = pd::query_pd_port_state(&handle.ec, index as u8);
        let connected = !matches!(pd.c_state, FrameworkPdTypeCState::Nothing);

        if connected {
            let flags = pd_flags(&pd);
            let base = module_descriptor(
                FrameworkModuleIdentity::UnknownUsbCOccupant,
                FrameworkModuleBus::Ec,
                FrameworkModuleSlotKind::UsbCExpansionCardSlot,
                if pd.alt_mode_flags & 0x03 != 0 {
                    FrameworkModuleConfidence::DerivedStrong
                } else {
                    FrameworkModuleConfidence::DerivedWeak
                },
                true,
                index as i32,
                flags,
                0,
                0,
                -1,
            );
            usb_slots[index] = make_expansion_card(
                base,
                pd,
                FrameworkExpansionCardType::Unknown,
                FrameworkModuleConfidence::Unknown,
            );
        } else {
            usb_slots[index].slot_index = index as i32;
            usb_slots[index].slot_kind = FrameworkModuleSlotKind::UsbCExpansionCardSlot;
            usb_slots[index].pd = pd;
        }

        pd_states[index] = Some(pd);
    }

    let dp_slots: Vec<usize> = pd_states
        .iter()
        .enumerate()
        .filter_map(|(i, pd)| {
            pd.and_then(|p| {
                if p.alt_mode_flags & 0x03 != 0 {
                    Some(i)
                } else {
                    None
                }
            })
        })
        .collect();

    let expansion_cards = detect_expansion_cards_local();

    if expansion_cards.len() == 1 && dp_slots.len() == 1 {
        let card = &expansion_cards[0];
        let si = dp_slots[0];
        let identity = expansion_card_identity(card.product_id);
        let mut flags = usb_slots[si].flags;
        flags |= module_flag(FrameworkModuleFlag::Connected);
        let pd = usb_slots[si].pd;
        let base = module_descriptor(
            identity,
            FrameworkModuleBus::Composite,
            FrameworkModuleSlotKind::UsbCExpansionCardSlot,
            FrameworkModuleConfidence::Direct,
            true,
            si as i32,
            flags,
            card.vendor_id as u32,
            card.product_id as u32,
            -1,
        );
        usb_slots[si] = make_expansion_card(
            base,
            pd,
            expansion_card_type(identity),
            FrameworkModuleConfidence::Direct,
        );
    } else {
        for card in &expansion_cards {
            push_detached_module(
                detached,
                detached_count,
                module_descriptor(
                    expansion_card_identity(card.product_id),
                    FrameworkModuleBus::Hid,
                    FrameworkModuleSlotKind::Detached,
                    FrameworkModuleConfidence::Direct,
                    true,
                    -1,
                    module_flag(FrameworkModuleFlag::Connected),
                    card.vendor_id as u32,
                    card.product_id as u32,
                    -1,
                ),
            );
        }
    }

    let unassigned_non_dp: Vec<usize> = pd_states
        .iter()
        .enumerate()
        .filter_map(|(i, pd)| {
            pd.and_then(|p| {
                let connected = !matches!(p.c_state, FrameworkPdTypeCState::Nothing);
                let no_dp = p.alt_mode_flags & 0x03 == 0;
                let unknown = matches!(
                    usb_slots[i].identity,
                    FrameworkModuleIdentity::UnknownUsbCOccupant
                );
                if connected && no_dp && unknown {
                    Some(i)
                } else {
                    None
                }
            })
        })
        .collect();

    let audio_cards = detect_audio_cards_local();
    if audio_cards.len() == 1 && unassigned_non_dp.len() == 1 {
        let card = &audio_cards[0];
        let si = unassigned_non_dp[0];
        let mut flags = usb_slots[si].flags;
        flags |= module_flag(FrameworkModuleFlag::Connected);
        let pd = usb_slots[si].pd;
        let base = module_descriptor(
            FrameworkModuleIdentity::AudioExpansionCard,
            FrameworkModuleBus::Composite,
            FrameworkModuleSlotKind::UsbCExpansionCardSlot,
            FrameworkModuleConfidence::DerivedWeak,
            true,
            si as i32,
            flags,
            card.vendor_id as u32,
            card.product_id as u32,
            -1,
        );
        usb_slots[si] = make_expansion_card(
            base,
            pd,
            FrameworkExpansionCardType::Audio,
            FrameworkModuleConfidence::Direct,
        );
    } else {
        for card in &audio_cards {
            push_detached_module(
                detached,
                detached_count,
                module_descriptor(
                    FrameworkModuleIdentity::AudioExpansionCard,
                    FrameworkModuleBus::Usb,
                    FrameworkModuleSlotKind::Detached,
                    FrameworkModuleConfidence::Direct,
                    true,
                    -1,
                    module_flag(FrameworkModuleFlag::Connected),
                    card.vendor_id as u32,
                    card.product_id as u32,
                    -1,
                ),
            );
        }
    }

    assign_card_pass(
        usb_slots,
        detached,
        detached_count,
        &pd_states,
        &detect_ssd_cards_local(),
        FrameworkModuleIdentity::SsdExpansionCard,
        FrameworkModuleBus::Usb,
        FrameworkModuleConfidence::Direct,
        FrameworkModuleConfidence::Direct,
    );

    assign_card_pass(
        usb_slots,
        detached,
        detached_count,
        &pd_states,
        &detect_usb_a_cards_local(),
        FrameworkModuleIdentity::UsbAExpansionCard,
        FrameworkModuleBus::Usb,
        FrameworkModuleConfidence::DerivedWeak,
        FrameworkModuleConfidence::DerivedWeak,
    );

    assign_card_pass(
        usb_slots,
        detached,
        detached_count,
        &pd_states,
        &detect_ethernet_cards_local(),
        FrameworkModuleIdentity::EthernetExpansionCard,
        FrameworkModuleBus::Usb,
        FrameworkModuleConfidence::DerivedWeak,
        FrameworkModuleConfidence::DerivedWeak,
    );

    assign_card_pass(
        usb_slots,
        detached,
        detached_count,
        &pd_states,
        &detect_sd_cards_local(),
        FrameworkModuleIdentity::SdExpansionCard,
        FrameworkModuleBus::Usb,
        FrameworkModuleConfidence::DerivedWeak,
        FrameworkModuleConfidence::DerivedWeak,
    );

    assign_card_pass(
        usb_slots,
        detached,
        detached_count,
        &pd_states,
        &detect_microsd_cards_local(),
        FrameworkModuleIdentity::MicroSdExpansionCard,
        FrameworkModuleBus::Usb,
        FrameworkModuleConfidence::DerivedWeak,
        FrameworkModuleConfidence::DerivedWeak,
    );

    // Apply the static per-platform slot capability (data lane / DisplayPort / charging). Slots the board does
    // not wire for PD (e.g. FW16 slots 3 & 6 at 900 mA) report a garbage/Invalid PD state from the EC, so clear
    // it here — they are surfaced by capability alone, not as broken PD ports.
    for (index, slot) in usb_slots
        .iter_mut()
        .enumerate()
        .take(usb_c_slot_count as usize)
    {
        let capability = super::capabilities::slot_capability(platform, index);
        slot.capability = capability;
        if capability.known != 0 && capability.supports_pd == 0 {
            slot.pd = pd::default_pd_port_state();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn assign_card_pass(
    usb_slots: &mut [FrameworkExpansionCardModuleDescriptor; MAX_USB_C_SLOT_COUNT],
    detached: &mut [FrameworkModuleDescriptor; 4],
    detached_count: &mut u8,
    pd_states: &[Option<FrameworkEcPdPortState>; MAX_USB_C_SLOT_COUNT],
    cards: &[super::detect::UsbModuleObservation],
    identity: FrameworkModuleIdentity,
    bus: FrameworkModuleBus,
    slot_confidence: FrameworkModuleConfidence,
    card_confidence: FrameworkModuleConfidence,
) {
    let candidates: Vec<usize> = pd_states
        .iter()
        .enumerate()
        .filter_map(|(i, pd)| {
            pd.and_then(|p| {
                let connected = !matches!(p.c_state, FrameworkPdTypeCState::Nothing);
                let no_dp = p.alt_mode_flags & 0x03 == 0;
                let unknown = matches!(
                    usb_slots[i].identity,
                    FrameworkModuleIdentity::UnknownUsbCOccupant
                );
                if connected && no_dp && unknown {
                    Some(i)
                } else {
                    None
                }
            })
        })
        .collect();

    if cards.len() == 1 && candidates.len() == 1 {
        let card = &cards[0];
        let si = candidates[0];
        let mut flags = usb_slots[si].flags;
        flags |= module_flag(FrameworkModuleFlag::Connected);
        let pd = usb_slots[si].pd;
        let base = module_descriptor(
            identity,
            bus,
            FrameworkModuleSlotKind::UsbCExpansionCardSlot,
            slot_confidence,
            true,
            si as i32,
            flags,
            card.vendor_id as u32,
            card.product_id as u32,
            -1,
        );
        usb_slots[si] =
            make_expansion_card(base, pd, expansion_card_type(identity), card_confidence);
    } else {
        for card in cards {
            push_detached_module(
                detached,
                detached_count,
                module_descriptor(
                    identity,
                    bus,
                    FrameworkModuleSlotKind::Detached,
                    card_confidence,
                    true,
                    -1,
                    module_flag(FrameworkModuleFlag::Connected),
                    card.vendor_id as u32,
                    card.product_id as u32,
                    -1,
                ),
            );
        }
    }
}
