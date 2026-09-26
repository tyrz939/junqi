//! PORT.md §6.m stage 16: the template harness. Carries `jane/test/templates.test.ts` "every
//! template proves its grants and its blocks, alone, from every door, in every transform" and
//! "the harness really catches a variant that opens a way round its own lock". The parse and
//! lint halves of that file moved to the build (`jane-schema`'s room lint).

use jane_data::{MissionDef, MissionNode, catalog};
use jane_world::dungeon::harness::{every_template, prove_template};

#[test]
fn every_template_proves_its_grants_and_blocks_alone_from_every_door_in_every_transform() {
    let d = &catalog().dungeons;
    let all = every_template();
    let mut proven: Vec<jane_core::TemplateId> = Vec::new();
    let mut shapes = 0;
    let mut failures = Vec::new();
    for &(m, node, t) in &all {
        let errors = prove_template(m, node, t);
        if !proven.contains(&t) {
            proven.push(t);
            shapes += d.template(t).shapes.len();
        }
        for e in errors {
            failures.push(format!("{} as {}.{}: {e}", d.template(t).id, m.id, m.nodes[node].id));
        }
    }
    // Every template the build knows is in some mission's pool, so every one was proven.
    assert_eq!(proven.len(), d.templates.len(), "templates no node can hold are never proven");
    assert!(d.templates.len() >= 100, "{} templates", d.templates.len());
    assert!(shapes >= d.templates.len());
    assert!(failures.is_empty(), "{} failure(s):\n{}", failures.len(), failures.join("\n"));
}

/// A mission with one node's holding changed, for as long as the test runs.
fn with_holding(
    m: &'static MissionDef,
    node: usize,
    socket: &str,
    change: impl Fn(&mut jane_data::MissionHolding),
) -> &'static MissionDef {
    let mut holds = m.nodes[node].holds.to_vec();
    for h in &mut holds {
        if h.socket == socket {
            change(h);
        }
    }
    let mut nodes = m.nodes.to_vec();
    nodes[node] = MissionNode { holds: Box::leak(holds.into_boxed_slice()), ..nodes[node] };
    Box::leak(Box::new(MissionDef { nodes: Box::leak(nodes.into_boxed_slice()), ..*m }))
}

#[test]
fn the_harness_really_catches_a_variant_that_opens_a_way_round_its_own_lock() {
    let d = &catalog().dungeons;
    let m = d.mission_of(jane_core::ZoneId::Mine).expect("the mine");
    let node = m.node_index("plate").expect("the plate room");
    let t = d.pool(m.nodes[node].pool).templates[0];
    assert_eq!(prove_template(m, node, t), Vec::<String>::new());
    // The same room, but the mission forgets to lock the chest: `chest:reward until plate:main` no longer holds.
    let careless = with_holding(m, node, "chest:reward", |h| h.locked = false);
    let errors = prove_template(careless, node, t).join("\n");
    assert!(errors.contains("chest:reward opens without plate:main"), "{errors}");
}
