//! `View::ground_digest`: what a console's kept county is keyed on (the presenter keeps its
//! county's houses and chunks while she is indoors). Out and back with the hours turning (the
//! county's clock rows stamp its props, moving `flags_generation`) the digest holds; a tile of
//! the county changed moves it.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::{CellIx, Tile, ZoneId};
use jane_data::catalog;
use jane_sim::input::DevOp;
use jane_sim::{Command, Seat};

#[test]
fn the_ground_digest_holds_out_and_back_and_moves_with_a_tile() {
    let mut s = new_game();
    let start = jane_sim::sym::of_name(catalog().name_id("start").unwrap());
    let at = |s: &jane_sim::Sim| {
        let v = s.view(Seat(0)).unwrap();
        assert_eq!(v.zone(), ZoneId::County);
        (v.ground_digest(), v.flags_generation())
    };
    let (d0, g0) = at(&s);
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: start }));
    for hour in [17, 19, 21, 23] {
        cmd(&mut s, Command::Dev(DevOp::Time { hour }));
        events(&mut s);
    }
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: start }));
    events(&mut s);
    let (d1, g1) = at(&s);
    assert_eq!(d1, d0, "the county's ground is as she left it (flags {g0} -> {g1})");
    s.state_mut().zone_mut(ZoneId::County).unwrap().tile_deltas.insert(CellIx(3), Tile::Wall);
    assert_ne!(at(&s).0, d1, "a changed tile moves the digest");
}
