//! The save (ARCHITECTURE.md §3.5): a header that reads without the body, refusals that do not
//! panic, and what loading does to the table. Carries `sim.test.ts` "rejects garbage and newer
//! saves without throwing" and `coop.test.ts`'s "a save is the host's world".

mod common;

use jane_core::ZoneId;
use jane_sim::save::{MAGIC, decode_state, read_header};
use jane_sim::{ClientToken, Command, SaveError, Seat, Sim, StampedCommand, StepInput};

#[test]
fn the_header_reads_without_the_body() {
    let s = common::new_game();
    let bytes = s.save();
    assert_eq!(bytes[..4], MAGIC);
    let (h, _) = read_header(&bytes).unwrap();
    assert_eq!(h.save_version, jane_sim::state::SAVE_VERSION);
    assert_eq!(h.content_hash, jane_data::catalog().content_hash);
    assert_eq!(h.summary.zone, ZoneId::County);
    assert_eq!((h.summary.day, h.summary.hour), (0, 17));
    assert!(h.summary.hp.0 > 0 && h.summary.hp == h.summary.max_hp);
    // The seed rebuilds the terrain, so tiles are never saved; but every unit and prop is saved
    // whole, even untouched. With the county furnished (two thousand creatures, thousands of
    // props) a fresh save is about 360 KB. ARCHITECTURE.md §3.5 budgets ~150 KB for a busy county:
    // saving units and props as deltas from their blueprint spawn is the P4 follow-up that gets
    // there. Until then this bound only catches a regression.
    assert!(bytes.len() < 512 * 1024, "{} bytes", bytes.len());
}

#[test]
fn garbage_and_foreign_saves_are_refused_without_a_panic() {
    let bytes = common::new_game().save();
    assert!(matches!(Sim::from_save_with(b"{not a save", common::bps()), Err(SaveError::NotASave)));
    assert!(Sim::from_save_with(&bytes[..20], common::bps()).is_err());
    assert!(Sim::from_save_with(&bytes[..bytes.len() - 9], common::bps()).is_err());
    // Another version.
    let mut v = bytes.clone();
    v[8] = v[8].wrapping_add(1);
    assert!(matches!(decode_state(&v), Err(SaveError::Version(_) | SaveError::Decode(_))));
    // Other content: the header's hash is the pin.
    let (mut h, body) = read_header(&bytes).unwrap();
    h.content_hash ^= 1;
    let head = postcard::to_allocvec(&h).unwrap();
    let mut other = MAGIC.to_vec();
    other.extend_from_slice(&(head.len() as u32).to_le_bytes());
    other.extend_from_slice(&head);
    other.extend_from_slice(body);
    assert!(matches!(decode_state(&other), Err(SaveError::ContentDrift { .. })));
}

#[test]
fn a_save_is_the_hosts_world() {
    let mut s = common::new_game();
    let cmds = [StampedCommand { seat: Some(Seat(0)), seq: 0, cmd: Command::Open(true) }];
    s.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
    let cmds = [StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: ClientToken(9) } }];
    s.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
    assert_eq!(s.state().party_size(), 2);
    let guest = s.state().players[1].unit;
    let loaded = Sim::from_save_with(&s.save(), common::bps()).unwrap();
    assert!(!loaded.state().open, "a save always loads closed");
    assert_eq!(loaded.state().party_size(), 1);
    assert_eq!(loaded.state().players[1].parked.as_ref().map(|u| u.id), Some(guest));
    assert!(loaded.state().zone(ZoneId::County).unwrap().unit(guest).is_none());
    assert!(loaded.view(Seat(1)).is_none());
    assert!(loaded.view(Seat(0)).is_some());
}
