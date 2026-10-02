// The names `data/bindings.json` and `config.json` use, shared by `build.rs` (which compiles the
// table and refuses a name it does not know) and `input.rs` (which reads the player's overrides).
// Plain data: no type of the crate, so the build script can `include!` it.

/// Actions: the data name, the Rust that builds it, and the label the Controls screen shows.
pub const ACTION_NAMES: &[(&str, &str, &str)] = &[
    ("up", "Action::Up", "Walk up"),
    ("down", "Action::Down", "Walk down"),
    ("left", "Action::Left", "Walk left"),
    ("right", "Action::Right", "Walk right"),
    ("sprint", "Action::Sprint", "Sprint"),
    ("hop", "Action::Hop", "Hop"),
    ("use", "Action::Use", "Use, talk, push"),
    ("bar1", "Action::Bar(0)", "Bar 1"),
    ("bar2", "Action::Bar(1)", "Bar 2"),
    ("bar3", "Action::Bar(2)", "Bar 3"),
    ("bar4", "Action::Bar(3)", "Bar 4"),
    ("bar5", "Action::Bar(4)", "Bar 5"),
    ("bar6", "Action::Bar(5)", "Bar 6"),
    ("bar7", "Action::Bar(6)", "Bar 7"),
    ("bar8", "Action::Bar(7)", "Bar 8"),
    ("bags", "Action::Bags", "Bag"),
    ("book", "Action::Book", "Book"),
    ("quests", "Action::Quests", "Log"),
    ("map", "Action::Map", "Map"),
    ("pause", "Action::Pause", "Pause, back"),
    ("quicksave", "Action::QuickSave", "Quick save"),
    ("quickload", "Action::QuickLoad", "Quick load"),
    ("console", "Action::Console", "Terminal"),
    ("debug", "Action::Debug", "Perf overlay"),
    ("grid", "Action::Grid", "World overlay"),
    ("shot", "Action::Shot", "Screenshot"),
    ("step", "Action::Step", "Hold, step a tick"),
    ("slow", "Action::Slow", "Quarter speed"),
    ("fast", "Action::Fast", "Four times speed"),
];

/// Keys: the cap name and the SDL scancode (USB HID usage id).
#[rustfmt::skip]
pub const KEY_NAMES: &[(&str, u16)] = &[
    ("A", 4), ("B", 5), ("C", 6), ("D", 7), ("E", 8), ("F", 9), ("G", 10), ("H", 11), ("I", 12),
    ("J", 13), ("K", 14), ("L", 15), ("M", 16), ("N", 17), ("O", 18), ("P", 19), ("Q", 20),
    ("R", 21), ("S", 22), ("T", 23), ("U", 24), ("V", 25), ("W", 26), ("X", 27), ("Y", 28),
    ("Z", 29), ("1", 30), ("2", 31), ("3", 32), ("4", 33), ("5", 34), ("6", 35), ("7", 36),
    ("8", 37), ("9", 38), ("0", 39), ("Enter", 40), ("Esc", 41), ("Back", 42), ("Tab", 43),
    ("Space", 44), ("-", 45), ("=", 46), ("[", 47), ("]", 48), ("\\", 49), (";", 51),
    ("'", 52), ("`", 53), (",", 54), (".", 55), ("/", 56), ("Caps", 57), ("F1", 58),
    ("F2", 59), ("F3", 60), ("F4", 61), ("F5", 62), ("F6", 63), ("F7", 64), ("F8", 65),
    ("F9", 66), ("F10", 67), ("F11", 68), ("F12", 69), ("Ins", 73), ("Home", 74), ("PgUp", 75),
    ("Del", 76), ("End", 77), ("PgDn", 78), ("→", 79), ("←", 80), ("↓", 81), ("↑", 82),
    ("KpEnter", 88), ("Ctrl", 224), ("Shift", 225), ("Alt", 226), ("RCtrl", 228),
    ("RShift", 229), ("AltGr", 230),
];

/// Pad inputs: the hint's name and the Rust that builds it.
pub const PAD_NAMES: &[(&str, &str)] = &[
    ("A", "PadInput::Button(pad::A)"),
    ("B", "PadInput::Button(pad::B)"),
    ("X", "PadInput::Button(pad::X)"),
    ("Y", "PadInput::Button(pad::Y)"),
    ("View", "PadInput::Button(pad::BACK)"),
    ("Menu", "PadInput::Button(pad::START)"),
    ("LS", "PadInput::Button(pad::LSTICK)"),
    ("RS", "PadInput::Button(pad::RSTICK)"),
    ("LB", "PadInput::Button(pad::LB)"),
    ("RB", "PadInput::Button(pad::RB)"),
    ("D↑", "PadInput::Button(pad::DPAD_UP)"),
    ("D↓", "PadInput::Button(pad::DPAD_DOWN)"),
    ("D←", "PadInput::Button(pad::DPAD_LEFT)"),
    ("D→", "PadInput::Button(pad::DPAD_RIGHT)"),
    ("LT", "PadInput::LeftTrigger"),
    ("RT", "PadInput::RightTrigger"),
];

/// Mouse buttons: the name and the Rust that builds it.
pub const MOUSE_NAMES: &[(&str, &str)] = &[
    ("Left click", "MouseButton::Left"),
    ("Middle click", "MouseButton::Middle"),
    ("Right click", "MouseButton::Right"),
];
