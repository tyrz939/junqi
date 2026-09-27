//! `jane sheet ui`: the UI's screens drawn headless through the presenter and `soft`, one PNG
//! each, in states a player reaches only by playing (the death veil, a fight's target frame and
//! chips, three toasts at once, a tooltip, a popover, a drag in flight, a choice). The review
//! tool for the UI as `jane sheet scene` is for the world (PRESENTATION.md §3, §6).

use std::path::Path;

use jane_present::input::Bindings;
use jane_present::text::Tone;
use jane_present::ui::core::{DragPayload, UiInput};
use jane_present::ui::dialogue::{self, DialogueBox};
use jane_present::ui::hud::{self, HudCtx};
use jane_present::ui::lan::{self, HostInfo, HostState, JoinInfo, JoinState, LanRow};
use jane_present::ui::menus::{self, MenuState, PauseInfo};
use jane_present::ui::title::{self, TitleInfo, TitleState};
use jane_present::ui::window::{self, WindowState};
use jane_present::ui::{Ui, UiOut};
use jane_present::view::{DialogueView, StatusChip, TargetFrame, ViewBuffers};
use jane_present::{Backend, Present, Tier};
use jane_render_soft::Soft;
use jane_sim::input::{InputFrame, StepInput};
use jane_sim::tuning::MAX_PLAYERS;
use jane_sim::{Seat, Sim};

/// The screens, by name.
pub const SCREENS: [&str; 10] =
    ["hud", "dead", "choice", "tooltip", "popover", "drag", "pause", "host", "join", "table"];

struct Rig {
    sim: Sim,
    present: Present,
    soft: Soft,
    ui: Ui,
    bufs: ViewBuffers,
}

impl Rig {
    fn new(seed: u32) -> Rig {
        let mut sim = Sim::new_game(seed, "Jane");
        let mut present = Present::new(Tier::T0);
        let mut soft = Soft::new();
        soft.upload_atlas(present.atlas());
        let ui = Ui::new(present.ui_art().clone());
        let mut bufs = ViewBuffers::new();
        for _ in 0..90 {
            sim.step(&StepInput { frames: [InputFrame::IDLE; MAX_PLAYERS], commands: &[] });
            let events = sim.drain_events().to_vec();
            let v = sim.view(Seat(0)).expect("seat 0");
            present.tick(&v, &events);
            bufs.tick(&v, &events);
        }
        Rig { sim, present, soft, ui, bufs }
    }

    /// Draws one frame: the world, then `ui_fn` over it at presenter tick `tick`.
    fn frame(&mut self, input: UiInput, tick: u32, ui_fn: impl FnOnce(&mut Ui, &ViewBuffers, &Sim)) -> Vec<UiOut> {
        let canvas = (768, 432);
        self.present.draw(128, canvas);
        self.ui.begin(input, tick, canvas);
        self.ui.draw_cursor = true;
        ui_fn(&mut self.ui, &self.bufs, &self.sim);
        let out = self.ui.out.clone();
        self.ui.finish(self.present.frame_mut());
        self.soft.draw(self.present.frame());
        out
    }

    fn write(&mut self, dir: &Path, name: &str) -> Result<(), String> {
        let mut px = Vec::new();
        let (w, h) = self.soft.read_back(&mut px);
        let mut rgba = Vec::with_capacity(px.len() * 4);
        for p in &px {
            let [_, r, g, b] = p.to_be_bytes();
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(format!("ui-sheet-{name}.png"));
        std::fs::write(&path, jane_art::sheet::png(u32::from(w), u32::from(h), &rgba))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{}", path.display());
        Ok(())
    }
}

fn at(p: (i32, i32)) -> UiInput {
    UiInput { pointer: Some(p), ..UiInput::default() }
}

/// Writes every screen (or those named) into `dir`.
pub fn run(dir: &Path, names: &[String]) -> Result<(), String> {
    if let Some(bad) = names.iter().find(|n| !SCREENS.contains(&n.as_str())) {
        return Err(format!("no screen {bad}: {}", SCREENS.join(", ")));
    }
    let want = |s: &str| names.is_empty() || names.iter().any(|n| n == s);
    let bind = Bindings::default();
    let cx = HudCtx { bindings: &bind, pad: false, window_open: false };
    let win_cx = HudCtx { window_open: true, ..cx };
    let mut rig = Rig::new(7);
    // A fight's HUD: a target, two chips, three toasts at once, a flash on the bar, the lag.
    {
        let b = &mut rig.bufs;
        b.hud.target = Some(TargetFrame { name: "Yard skeleton".into(), hp: b.hud.hp, hostile: true, boss: false });
        if let Some(t) = &mut b.hud.target {
            t.hp.frac = 420;
            t.hp.lag = 610;
        }
        let cat = jane_data::catalog();
        for (k, id) in ["chilled", "burning"].iter().enumerate() {
            if let Some(e) = cat.combat.effect_id(id) {
                let d = cat.combat.effect(e);
                b.hud.statuses.push(StatusChip {
                    icon: d.icon,
                    name: d.name,
                    ticks_left: 200 + k as u32 * 300,
                    total: 600,
                    harmful: true,
                });
            }
        }
        b.push_toast("Rats 3 of 5", Tone::Good);
        b.push_toast("My bag is full", Tone::Refused);
        b.push_toast("It shifts a little. Hold to push it.", Tone::Plain);
        b.push_toast("My bag is full", Tone::Refused);
        b.hud.hp.frac = 310;
        b.hud.hp.lag = 540;
        b.hud.hp.now = 46;
        b.hud.bar[0].flash = 8;
        b.hud.bar[0].cooldown = 620;
        b.hud.bar[6].gcd = 400;
        b.hud.prompt = Some(jane_present::view::Prompt { verb: "Talk", label: "The dog".into(), hold: false });
    }
    // The toasts arrived a few ticks ago: risen and faded in.
    rig.bufs.tick += 12;
    let tick = rig.bufs.tick;
    if want("hud") {
        rig.frame(at((520, 170)), tick, |ui, b, _| hud::draw(ui, b, cx));
        rig.write(dir, "hud")?;
    }
    if want("dead") {
        let mut b = rig.bufs.clone();
        b.me.dead = true;
        b.me.respawn_ticks = 185;
        b.hud.hp.frac = 0;
        b.hud.toasts.clear();
        rig.frame(UiInput::default(), tick, |ui, _, _| hud::draw(ui, &b, cx));
        rig.write(dir, "dead")?;
    }
    if want("choice") {
        let d = DialogueView {
            speaker: "The dog".into(),
            text: "You keep coming back to look. That is a city habit. Will you take the key or not?".into(),
            options: vec!["I'll take it.".into(), "Not yet.".into()],
            choosing: true,
            more: false,
            key: (1, 2, 3, 4),
        };
        let mut bx = DialogueBox::default();
        let mut b = rig.bufs.clone();
        b.hud.toasts.clear();
        b.hud.prompt = None;
        rig.frame(UiInput::default(), 1000, |ui, _, _| {
            hud::draw(ui, &b, cx);
            dialogue::draw(ui, &mut bx, &d, cx);
        });
        rig.frame(at((300, 330)), 1100, |ui, _, _| {
            hud::draw(ui, &b, cx);
            dialogue::draw(ui, &mut bx, &d, cx);
        });
        rig.write(dir, "choice")?;
    }
    let mut b = rig.bufs.clone();
    b.hud.toasts.clear();
    // The bag's first slot, as the window lays it out.
    let slot0 =
        (i32::from(window::rect((768, 432)).x) + 12 + 8 + 18, i32::from(window::rect((768, 432)).y) + 42 + 4 + 22 + 18);
    if want("tooltip") {
        let mut st = WindowState::default();
        for t in 0..30 {
            rig.frame(at(slot0), 2000 + t, |ui, _, sim| {
                hud::draw(ui, &b, win_cx);
                let v = sim.view(Seat(0));
                window::draw(ui, &mut st, &b, v.as_ref(), win_cx);
            });
        }
        rig.write(dir, "tooltip")?;
    }
    if want("popover") {
        let mut st = WindowState::default();
        let right = UiInput { pointer: Some(slot0), right_pressed: true, ..UiInput::default() };
        rig.frame(right, 3000, |ui, _, sim| {
            hud::draw(ui, &b, win_cx);
            window::draw(ui, &mut st, &b, sim.view(Seat(0)).as_ref(), win_cx);
        });
        rig.frame(at((slot0.0 + 20, slot0.1 + 30)), 3001, |ui, _, sim| {
            hud::draw(ui, &b, win_cx);
            window::draw(ui, &mut st, &b, sim.view(Seat(0)).as_ref(), win_cx);
        });
        rig.write(dir, "popover")?;
    }
    if want("drag") {
        let mut st = WindowState::default();
        let press = UiInput { pointer: Some(slot0), held: true, pressed: true, ..UiInput::default() };
        rig.frame(press, 4000, |ui, _, sim| {
            hud::draw(ui, &b, win_cx);
            window::draw(ui, &mut st, &b, sim.view(Seat(0)).as_ref(), win_cx);
        });
        let over_bar = hud::bar_rect((768, 432));
        let to = (i32::from(over_bar.x) + 8 + 3 * 40 + 18, i32::from(over_bar.y) + 6 + 18);
        for (k, p) in [(slot0.0 + 30, slot0.1 + 40), (to.0 - 40, to.1 - 60), to].iter().enumerate() {
            let hold = UiInput { pointer: Some(*p), held: true, ..UiInput::default() };
            rig.frame(hold, 4001 + k as u32, |ui, _, sim| {
                hud::draw(ui, &b, win_cx);
                window::draw(ui, &mut st, &b, sim.view(Seat(0)).as_ref(), win_cx);
            });
        }
        let dragging = rig.ui.drag.is_some_and(|d| d.moved && d.payload == DragPayload::Bag(0));
        if !dragging {
            return Err("the drag did not start".into());
        }
        rig.write(dir, "drag")?;
    }
    if want("pause") {
        let mut st = MenuState::default();
        let info = PauseInfo {
            can_save: true,
            when: "Day 1, 18:42",
            zone: "The Lowfields",
            company: false,
            lan: Some(("Open to LAN", true)),
            guest: false,
        };
        rig.frame(at((384, 180)), 5000, |ui, _, _| {
            hud::draw(ui, &b, cx);
            ui.interactive = true;
            menus::pause(ui, &mut st, &info);
        });
        rig.write(dir, "pause")?;
    }
    // Playing together (P8): the title's Host and Join, and the table on the HUD.
    let slots = [Some("The Lowfields · Day 2, 21:00".to_owned()), None, Some("Julie's house · Day 5, 08:00".into())];
    if want("host") {
        let mut t = TitleState { name: "Tess".into(), ..TitleState::default() };
        let mut st = HostState { port_text: "7777".into(), ..HostState::default() };
        st.choice.slot = Some(0);
        st.choice.seats = 3;
        for k in 0..2 {
            rig.frame(at((330, 262)), 6000 + k, |ui, _, _| {
                ui.interactive = false;
                title::draw(ui, &mut t, TitleInfo { has_save: true });
                ui.interactive = true;
                lan::host(ui, &mut st, &HostInfo { slots: &slots, port: 7777 });
            });
        }
        rig.write(dir, "host")?;
    }
    if want("join") {
        let mut t = TitleState::default();
        let mut st = JoinState { addr: "192.168.1.20".into(), ..JoinState::default() };
        let found = [
            LanRow { name: "Tess's world".into(), addr: "192.168.1.20:7777".into(), seats: "2 of 4".into(), ok: true },
            LanRow {
                name: "Jane's world (served)".into(),
                addr: "192.168.1.31:7777".into(),
                seats: "1 of 4".into(),
                ok: true,
            },
            LanRow { name: "Mo's world".into(), addr: "192.168.1.44:7777".into(), seats: "4 of 4".into(), ok: false },
        ];
        let refused = "Refused: the host's content is a3516ef3a75d37a1, this build's is a3516ef3a75de90c: both must run the same data";
        let info = JoinInfo { found: &found, status: Some((refused, true)), joining: false };
        rig.frame(at((300, 150)), 7000, |ui, _, _| {
            ui.interactive = false;
            title::draw(ui, &mut t, TitleInfo { has_save: true });
            ui.interactive = true;
            lan::join(ui, &mut st, &info);
        });
        rig.write(dir, "join")?;
    }
    if want("table") {
        rig.frame(UiInput::default(), 8000, |ui, b, _| {
            hud::draw(ui, b, cx);
            lan::table(ui, 0b0111, 1, false, true);
            lan::stall(ui, 0b0100, 3400, false);
        });
        rig.write(dir, "table")?;
    }
    Ok(())
}
