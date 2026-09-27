fn main() {
    let bps = jane_sim::Blueprints::build(1).unwrap();
    let cat = jane_data::catalog();
    for z in jane_core::ZoneId::ALL {
        let bp = bps.get(z);
        let names: Vec<String> = bp
            .marks
            .keys()
            .take(8)
            .map(|k| match k {
                jane_core::ids::Key::Name(n) => cat.name(*n).to_string(),
                jane_core::ids::Key::Local(i) => bp.local_names.get(*i as usize).cloned().unwrap_or_default(),
            })
            .collect();
        println!("{} indoor={} ambient={:?}: {}", z.name(), bp.indoor, bp.ambient, names.join(", "));
    }
}
