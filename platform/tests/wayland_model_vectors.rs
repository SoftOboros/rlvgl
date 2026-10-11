// SPDX-License-Identifier: MIT
//! Supplemental host-independent WLD-01 acceptance vectors.
//!
//! These exercise the production model directly, including on Apple hosts.
//! They do not emulate a compositor or prove SHM release/session behavior.

extern crate alloc;

#[allow(dead_code)]
#[path = "../src/wayland/model.rs"]
mod model;

use model::{AllocationAdmission, DamageSet, Geometry, SubmissionState};
use rlvgl_core::widget::{Color, Rect};

#[test]
fn partial_shadow_updates_preserve_neighbors_across_two_and_three_slots() {
    let geometry = Geometry::checked((3, 1), (3, 1), 1).unwrap();
    for slot_count in [2, 3] {
        let mut slots = vec![vec![0xa5; geometry.slot_bytes]; slot_count];
        let mut shadow = [Color(1, 2, 3, 0), Color(4, 5, 6, 0), Color(7, 8, 9, 0)];
        for frame in 0..(slot_count * 3 + 1) {
            // One logical pixel changes; the other two retain prior values.
            let changed = frame % shadow.len();
            shadow[changed] = Color((frame + 20) as u8, 21, 22, 0);
            let slot = &mut slots[frame % slot_count];
            model::encode_complete_frame(geometry, &shadow, slot).unwrap();
            for (index, Color(red, green, blue, _)) in shadow.iter().enumerate() {
                assert_eq!(
                    &slot[index * 4..index * 4 + 4],
                    &[*blue, *green, *red, 0xff]
                );
            }
        }
    }
}

#[test]
fn clipped_damage_is_scaled_once_inside_letterbox() {
    let geometry = Geometry::checked((4, 3), (8, 5), 3).unwrap();
    assert_eq!(
        geometry.map_damage(Rect {
            x: -2,
            y: -1,
            width: 4,
            height: 3
        }),
        Some((6, 3, 6, 6))
    );
    assert_eq!(
        geometry.map_damage(Rect {
            x: 3,
            y: 2,
            width: 9,
            height: 9
        }),
        Some((15, 9, 3, 3))
    );
    assert_eq!(
        geometry.map_damage(Rect {
            x: 4,
            y: 0,
            width: 1,
            height: 1
        }),
        None
    );
}

#[test]
fn repeated_configure_budget_waits_for_each_retired_generation() {
    // Three proposed resizes all remain blocked at this fixed aggregate budget.
    // Releasing one generation is insufficient; releasing the second admits
    // the latest geometry at the exact limit, without adding a slot.
    for steady in [60, 70, 80] {
        assert_eq!(
            model::classify_allocation(steady, 30, 20, 100).unwrap(),
            AllocationAdmission::Deferred {
                required: steady + 50
            }
        );
    }
    assert_eq!(
        model::classify_allocation(80, 10, 20, 100).unwrap(),
        AllocationAdmission::Deferred { required: 110 }
    );
    assert_eq!(
        model::classify_allocation(80, 0, 20, 100).unwrap(),
        AllocationAdmission::Ready { peak_bytes: 100 }
    );
}

#[test]
fn damage_overflow_stays_bounded_until_submitted_state_is_cleared() {
    let mut damage = DamageSet::new(2);
    for x in 0..1000 {
        damage.add(Rect {
            x,
            y: 0,
            width: 1,
            height: 1,
        });
        assert!(damage.rects.len() <= 2);
    }
    assert!(damage.full);
    assert!(damage.rects.is_empty());
    damage.clear();
    damage.add(Rect {
        x: 2,
        y: 0,
        width: 1,
        height: 1,
    });
    assert!(!damage.full);
    assert_eq!(damage.rects.len(), 1);
}

#[test]
fn resize_does_not_bypass_closed_frame_gate_or_revive_terminal_state() {
    let mut state = SubmissionState::new();
    state.request_present();
    state.submitted();
    state.request_present();
    state.reset_for_geometry();
    state.request_present();
    assert!(!state.can_probe_slot());
    state.frame_done();
    assert!(state.can_probe_slot());
    state.stop();
    state.reset_for_geometry();
    state.frame_done();
    state.request_present();
    assert!(!state.can_probe_slot());
}
