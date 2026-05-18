use crate::*;

use super::conversions::{expansion_card_identity, module_descriptor, module_flag};
use super::detect::{
    detect_audio_cards_local, detect_expansion_cards_local, push_detached_module,
    read_pd_port_observation, unknown_usb_c_descriptor,
};

pub(super) const MAX_USB_C_SLOT_COUNT: usize = 6;

pub(super) fn populate_usb_slots(
    handle: &FrameworkEcHandle,
    usb_c_slot_count: u8,
    usb_slots: &mut [FrameworkModuleDescriptor; MAX_USB_C_SLOT_COUNT],
    detached: &mut [FrameworkModuleDescriptor; 4],
    detached_count: &mut u8,
) {
    let mut pd_observations = [None; MAX_USB_C_SLOT_COUNT];
    for (index, slot) in usb_slots
        .iter_mut()
        .enumerate()
        .take(usb_c_slot_count as usize)
    {
        pd_observations[index] = read_pd_port_observation(&handle.ec, index as u8);
        if let Some(observation) = pd_observations[index] {
            if let Some(descriptor) = unknown_usb_c_descriptor(index, observation) {
                *slot = descriptor;
            }
        }
    }

    let expansion_cards = detect_expansion_cards_local();
    let dp_slots: Vec<usize> = pd_observations
        .iter()
        .enumerate()
        .filter_map(|(index, observation)| observation.filter(|obs| obs.dp_alt_mode).map(|_| index))
        .collect();

    if expansion_cards.len() == 1 && dp_slots.len() == 1 {
        let card = &expansion_cards[0];
        let slot_index = dp_slots[0];
        let mut flags = usb_slots[slot_index].flags;
        flags |= module_flag(FrameworkModuleFlag::Connected);
        usb_slots[slot_index] = module_descriptor(
            expansion_card_identity(card.product_id),
            FrameworkModuleBus::Composite,
            FrameworkModuleSlotKind::UsbCPort,
            FrameworkModuleConfidence::DerivedStrong,
            true,
            slot_index as i32,
            flags,
            card.vendor_id as u32,
            card.product_id as u32,
            -1,
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

    let audio_cards = detect_audio_cards_local();
    let audio_candidates: Vec<usize> = pd_observations
        .iter()
        .enumerate()
        .filter_map(|(index, observation)| {
            observation.and_then(|obs| {
                if obs.connected
                    && !obs.dp_alt_mode
                    && matches!(
                        usb_slots[index].identity,
                        FrameworkModuleIdentity::UnknownUsbCOccupant
                    )
                {
                    Some(index)
                } else {
                    None
                }
            })
        })
        .collect();

    if audio_cards.len() == 1 && audio_candidates.len() == 1 {
        let card = &audio_cards[0];
        let slot_index = audio_candidates[0];
        let mut flags = usb_slots[slot_index].flags;
        flags |= module_flag(FrameworkModuleFlag::Connected);
        usb_slots[slot_index] = module_descriptor(
            FrameworkModuleIdentity::AudioExpansionCard,
            FrameworkModuleBus::Composite,
            FrameworkModuleSlotKind::UsbCPort,
            FrameworkModuleConfidence::DerivedWeak,
            true,
            slot_index as i32,
            flags,
            card.vendor_id as u32,
            card.product_id as u32,
            -1,
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
}
