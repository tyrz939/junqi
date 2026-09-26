//! The least of a bag (`sim/inventory.ts bagAdd`), for the start kit and for handing story items
//! on. The inventory unit owns bags and grows this module.

use jane_core::{ItemId, Stack};

/// Add `qty` of `item`: top up its stacks in slot order, then fill empty slots. Returns how
/// many did not fit.
pub fn bag_add(bag: &mut [Option<Stack>], item: ItemId, qty: u16) -> u16 {
    let max = jane_data::catalog().combat.item(item).max_stack.max(1);
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

/// How many of `item` the bag holds.
pub fn bag_count(bag: &[Option<Stack>], item: ItemId) -> u32 {
    bag.iter().flatten().filter(|s| s.item == item).map(|s| u32::from(s.qty)).sum()
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
}
