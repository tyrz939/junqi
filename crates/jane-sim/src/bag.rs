//! A bag as slots (`sim/inventory.ts` bagAdd, bagCount, bagRemove, bagHasRoom, bagMove): 24
//! slots of `Option<Stack>`. Stack into existing stacks first, then the first hole; whatever does
//! not fit is returned, never silently dropped (the 2026 Phaser build ignored the leftover and
//! could lose the house key). Pure functions over the slots: events and the journal are the
//! caller's (`inventory.rs`).

use jane_core::{ItemId, Stack};

use crate::tuning::BAG_SLOTS;

fn max_stack(item: ItemId) -> u16 {
    jane_data::catalog().combat.item(item).max_stack.max(1)
}

/// Add `qty` of `item`: top up its stacks in slot order, then fill empty slots. Returns how
/// many did not fit.
pub fn bag_add(bag: &mut [Option<Stack>], item: ItemId, qty: u16) -> u16 {
    let max = max_stack(item);
    let mut left = qty;
    for s in bag.iter_mut().flatten() {
        if left == 0 {
            break;
        }
        if s.item == item && s.qty < max {
            let n = left.min(max - s.qty);
            s.qty += n;
            left -= n;
        }
    }
    for s in bag.iter_mut() {
        if left == 0 {
            break;
        }
        if s.is_none() {
            let n = left.min(max);
            *s = Some(Stack { item, qty: n });
            left -= n;
        }
    }
    left
}

/// Into her bag and ring (`PlayerState::bag`): what fits in the bag goes there; a key that does
/// not fit goes on the ring. Returns how many did not fit.
pub fn held_add(held: &mut [Option<Stack>], item: ItemId, qty: u16) -> u16 {
    let split = BAG_SLOTS.min(held.len());
    let left = bag_add(&mut held[..split], item, qty);
    if left > 0 && is_key(item) { bag_add(&mut held[split..], item, left) } else { left }
}

/// Would `qty` of `item` fit in her bag, or a key on her ring?
pub fn held_has_room(held: &[Option<Stack>], item: ItemId, qty: u16) -> bool {
    let split = BAG_SLOTS.min(held.len());
    bag_has_room(&held[..split], item, qty) || (is_key(item) && bag_has_room(&held[split..], item, qty))
}

/// What is on the ring comes into the bag's free slots. True if anything moved.
pub fn ring_settle(held: &mut [Option<Stack>]) -> bool {
    let split = BAG_SLOTS.min(held.len());
    let mut moved = false;
    for r in split..held.len() {
        let Some(s) = held[r] else { continue };
        let Some(to) = held[..split].iter().position(Option::is_none) else { break };
        held[to] = Some(s);
        held[r] = None;
        moved = true;
    }
    moved
}

/// A key: anything that opens a lock.
fn is_key(item: ItemId) -> bool {
    jane_data::catalog().combat.item(item).opens.is_some()
}

/// How many of `item` the bag holds.
pub fn bag_count(bag: &[Option<Stack>], item: ItemId) -> u32 {
    bag.iter().flatten().filter(|s| s.item == item).map(|s| u32::from(s.qty)).sum()
}

/// Take up to `qty` of `item`, last stacks first. Returns how many were taken.
pub fn bag_remove(bag: &mut [Option<Stack>], item: ItemId, qty: u16) -> u16 {
    let mut left = qty;
    for slot in bag.iter_mut().rev() {
        if left == 0 {
            break;
        }
        let Some(s) = slot else { continue };
        if s.item != item {
            continue;
        }
        let n = left.min(s.qty);
        s.qty -= n;
        left -= n;
        if s.qty == 0 {
            *slot = None;
        }
    }
    qty - left
}

/// Would `qty` of `item` fit?
pub fn bag_has_room(bag: &[Option<Stack>], item: ItemId, qty: u16) -> bool {
    let max = u32::from(max_stack(item));
    let mut room = 0u32;
    for s in bag {
        match s {
            None => room += max,
            Some(s) if s.item == item => room += max.saturating_sub(u32::from(s.qty)),
            Some(_) => {}
        }
        if room >= u32::from(qty) {
            return true;
        }
    }
    false
}

/// Drag one slot onto another: the same item merges as far as it stacks, anything else swaps.
/// Returns whether anything moved.
pub fn bag_move(bag: &mut [Option<Stack>], from: usize, to: usize) -> bool {
    if from == to || from >= bag.len() || to >= bag.len() {
        return false;
    }
    let Some(a) = bag[from] else { return false };
    match bag[to] {
        Some(mut b) if b.item == a.item => {
            let n = a.qty.min(max_stack(a.item).saturating_sub(b.qty));
            b.qty += n;
            bag[to] = Some(b);
            bag[from] = (a.qty > n).then_some(Stack { item: a.item, qty: a.qty - n });
        }
        _ => bag.swap(from, to),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tops_up_then_fills_and_says_what_is_left() {
        let cat = jane_data::catalog();
        let apple = cat.combat.item_id("apple").unwrap();
        let max = cat.combat.item(apple).max_stack;
        let mut bag = [None; 3];
        assert_eq!(bag_add(&mut bag, apple, 1), 0);
        assert_eq!(bag_add(&mut bag, apple, max), 0);
        assert_eq!(bag_count(&bag, apple), u32::from(max) + 1);
        assert_eq!(bag[0].unwrap().qty, max);
        assert_eq!(bag_add(&mut bag, apple, 3 * max), max * 3 - (max - 1) - max);
    }

    #[test]
    fn removes_last_stacks_first_and_moves_merge_or_swap() {
        let cat = jane_data::catalog();
        let apple = cat.combat.item_id("apple").unwrap();
        let rock = cat.combat.item_id("rock").unwrap();
        let max = cat.combat.item(apple).max_stack;
        let mut bag = [None; 4];
        bag_add(&mut bag, apple, max + 2);
        assert_eq!(bag_remove(&mut bag, apple, 1), 1);
        assert_eq!(bag[1].unwrap().qty, 1, "the last stack pays first");
        assert_eq!(bag_remove(&mut bag, apple, 5), 5);
        assert_eq!(bag[1], None);
        assert_eq!(bag_remove(&mut bag, rock, 1), 0);
        bag[3] = Some(Stack { item: rock, qty: 1 });
        assert!(bag_move(&mut bag, 3, 0));
        assert_eq!(bag[0].unwrap().item, rock);
        assert_eq!(bag[3].unwrap().item, apple);
        bag[1] = Some(Stack { item: apple, qty: 2 });
        assert!(bag_move(&mut bag, 1, 3));
        assert_eq!(bag[1], None);
        assert_eq!(bag[3].unwrap().qty, max - 4 + 2);
        assert!(bag_has_room(&bag, apple, max));
        assert!(!bag_move(&mut bag, 2, 0), "an empty slot moves nothing");
    }

    #[test]
    fn a_full_bag_takes_a_key_on_the_ring_and_lets_it_in_as_room_comes() {
        let cat = jane_data::catalog();
        let rock = cat.combat.item_id("rock").unwrap();
        let key = cat.combat.item_id("key_forest").unwrap();
        let max = cat.combat.item(rock).max_stack;
        let mut held = [None; crate::tuning::HELD_SLOTS];
        for _ in 0..BAG_SLOTS {
            held_add(&mut held, rock, max);
        }
        assert_eq!(held_add(&mut held, rock, 1), 1, "the ring holds keys alone");
        assert!(held_has_room(&held, key, 1));
        assert_eq!(held_add(&mut held, key, 1), 0, "a full bag never leaves a key behind");
        assert_eq!(held[BAG_SLOTS].unwrap().item, key);
        assert!(!ring_settle(&mut held), "no room yet");
        bag_remove(&mut held, rock, max);
        assert!(ring_settle(&mut held));
        assert_eq!(held[BAG_SLOTS - 1].unwrap().item, key, "into the slot the rocks left");
        assert_eq!(held[BAG_SLOTS], None);
    }
}
