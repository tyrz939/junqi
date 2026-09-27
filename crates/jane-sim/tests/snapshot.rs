//! What the lockstep host leans on (ARCHITECTURE.md §7): a snapshot of a live world loads as that
//! world (every seat where she sat, the door as open as it was) and plays on to the same hashes;
//! `seat_for` names the seat a join will give before the step that gives it.

mod common;

use common::{Tape, bps, new_game};
use jane_sim::{Command, Sim};

#[test]
fn a_snapshot_of_a_live_table_loads_as_that_table_and_plays_on_the_same() {
    let mut live = new_game();
    let mut tape = Tape::new(3);
    // Joiners, each loaded from a snapshot at a different frame and stepped from then on with
    // the live world's input.
    let mut joined: Vec<Sim> = Vec::new();
    let mut checked = 0;
    for f in 0..3000u32 {
        let input = tape.frame(f);
        live.step(&input);
        for j in &mut joined {
            j.step(&input);
        }
        if f % 700 == 699 {
            let b = Sim::from_snapshot_with(&live.save(), bps()).expect("the snapshot loads");
            assert_eq!(b.hash(), live.hash(), "frame {f}: the same world");
            assert_eq!(b.state().party_size(), live.state().party_size(), "nobody was parked");
            assert_eq!(b.state().open, live.state().open);
            joined.push(b);
        }
        if f % 100 == 99 {
            // One encoding, both answers.
            assert_eq!(live.save_and_hash(), (live.save(), live.hash()));
            // A snapshot encodes to the same, wherever it is encoded.
            let snap = live.snapshot();
            let off = std::thread::spawn(move || snap.save_and_hash()).join().unwrap();
            assert_eq!(off, live.save_and_hash());
            for j in &joined {
                assert_eq!(j.hash(), live.hash(), "frame {f}: a joiner plays on as the live world");
                checked += 1;
            }
        }
    }
    assert!(checked > 40, "{checked}");
    assert!(live.state().players.len() > 1, "the tape sat guests down");
}

#[test]
fn seat_for_names_the_seat_the_join_gives() {
    let mut s = new_game();
    let mut tape = Tape::new(5);
    let mut joins = 0;
    for f in 0..6000u32 {
        let input = tape.frame(f);
        let join = input.commands.iter().find_map(|c| match c.cmd {
            Command::Join { who } => Some(who),
            _ => None,
        });
        let predicted = join.map(|who| s.seat_for(who));
        let before: Vec<bool> = s.state().players.iter().map(|p| p.connected).collect();
        s.step(&input);
        let (Some(who), Some(predicted)) = (join, predicted) else { continue };
        // Only when the join is the frame's first seat change is the prediction the host's.
        if input.commands.iter().filter(|c| matches!(c.cmd, Command::Join { .. })).count() > 1 {
            continue;
        }
        joins += 1;
        let sat = s
            .state()
            .players
            .iter()
            .enumerate()
            .find(|(i, p)| p.connected && p.who == who && !before.get(*i).copied().unwrap_or(false))
            .map(|(_, p)| p.seat);
        assert_eq!(predicted, sat, "frame {f}: {who:?}");
    }
    assert!(joins > 10, "{joins}");
}
