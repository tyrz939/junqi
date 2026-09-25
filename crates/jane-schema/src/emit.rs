//! Rust source for a value: how `jane-data`'s build script turns the compiled catalog into
//! plain statics (PORT.md §5.5). The emitted file starts with [`PRELUDE`], so every type is
//! named bare.
//!
//! Every model struct implements [`Emit`] through [`model!`]; a field added to a struct is added
//! to its emitter by the same line, and a type the emitter does not know is a compile error in
//! `jane-data`, never a silently dropped field.

use std::fmt::Write;

use jane_core::action::{
    Action, CameraMode, Cond, Condition, CondsRef, Facing, FactKey, FlagKey, FlagOp, FlagTest, Heal, ListRef, NamesRef,
    School, Stack, Stat, TextRef, Thing,
};
use jane_core::blueprint::TriggerMode;
use jane_core::grid::{Cell, Rect};
use jane_core::ids::*;
use jane_core::num::{Fx, Milli, Permille, Q16, Tick};
use jane_core::{Angle, Tile};

/// The `use` lines the emitted source opens with.
pub const PRELUDE: &str = "#[allow(unused_imports)]\nuse jane_core::action::*;\n#[allow(unused_imports)]\nuse jane_core::blueprint::TriggerMode;\n#[allow(unused_imports)]\nuse jane_core::grid::{Cell, Rect};\n#[allow(unused_imports)]\nuse jane_core::ids::*;\n#[allow(unused_imports)]\nuse jane_core::num::{Fx, Milli, Permille, Q16, Tick};\n#[allow(unused_imports)]\nuse jane_core::{Angle, Tile};\n#[allow(unused_imports)]\nuse jane_schema::model::*;\n";

/// A value that can write itself as a Rust expression of its own type.
pub trait Emit {
    fn emit(&self, out: &mut String);
}

/// Emit to a fresh string.
pub fn to_rust<T: Emit + ?Sized>(v: &T) -> String {
    let mut s = String::new();
    v.emit(&mut s);
    s
}

macro_rules! emit_display {
    ($($t:ty),*) => {$(
        impl Emit for $t {
            fn emit(&self, out: &mut String) {
                let _ = write!(out, "{}", self);
            }
        }
    )*};
}
emit_display!(u8, u16, u32, u64, i8, i16, i32, i64, bool);

impl Emit for str {
    fn emit(&self, out: &mut String) {
        let _ = write!(out, "{self:?}");
    }
}

impl<T: Emit> Emit for Option<T> {
    fn emit(&self, out: &mut String) {
        match self {
            None => out.push_str("None"),
            Some(v) => {
                out.push_str("Some(");
                v.emit(out);
                out.push(')');
            }
        }
    }
}

impl<T: Emit> Emit for [T] {
    fn emit(&self, out: &mut String) {
        out.push_str("&[");
        for (i, v) in self.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            v.emit(out);
        }
        out.push(']');
    }
}

impl<T: Emit + ?Sized> Emit for &T {
    fn emit(&self, out: &mut String) {
        (**self).emit(out);
    }
}

impl<A: Emit, B: Emit> Emit for (A, B) {
    fn emit(&self, out: &mut String) {
        out.push('(');
        self.0.emit(out);
        out.push_str(", ");
        self.1.emit(out);
        out.push(')');
    }
}

impl<A: Emit, B: Emit, C: Emit> Emit for (A, B, C) {
    fn emit(&self, out: &mut String) {
        out.push('(');
        self.0.emit(out);
        out.push_str(", ");
        self.1.emit(out);
        out.push_str(", ");
        self.2.emit(out);
        out.push(')');
    }
}

impl<T: Emit, const N: usize> Emit for [T; N] {
    fn emit(&self, out: &mut String) {
        out.push('[');
        for (i, v) in self.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            v.emit(out);
        }
        out.push(']');
    }
}

/// Newtypes over one integer: `Name(n)`.
macro_rules! emit_newtype {
    ($($t:ident),*) => {$(
        impl Emit for $t {
            fn emit(&self, out: &mut String) {
                let _ = write!(out, concat!(stringify!($t), "({})"), self.0);
            }
        }
    )*};
}
emit_newtype!(
    SpellId,
    EffectId,
    ItemId,
    UnitDefId,
    PropDefId,
    QuestId,
    DialogueId,
    TriggerId,
    RecipeId,
    TextId,
    StoryId,
    ConsequenceId,
    SpriteId,
    DungeonId,
    PoolId,
    TemplateId,
    NameId,
    Sym,
    Fx,
    Milli,
    Permille,
    Q16,
    Tick,
    Angle
);

/// Fieldless enums: `Type::Variant` from the Debug name.
macro_rules! emit_unit_enum {
    ($($t:ident),*) => {$(
        impl Emit for $t {
            fn emit(&self, out: &mut String) {
                let _ = write!(out, concat!(stringify!($t), "::{:?}"), self);
            }
        }
    )*};
}
emit_unit_enum!(ZoneId, School, Stat, Facing, CameraMode, Tile, TriggerMode);

impl Emit for Cell {
    fn emit(&self, out: &mut String) {
        let _ = write!(out, "Cell {{ x: {}, y: {} }}", self.x, self.y);
    }
}

impl Emit for Rect {
    fn emit(&self, out: &mut String) {
        let _ = write!(out, "Rect {{ x: {}, y: {}, w: {}, h: {} }}", self.x, self.y, self.w, self.h);
    }
}

impl Emit for Key {
    fn emit(&self, out: &mut String) {
        match self {
            Key::Name(n) => {
                out.push_str("Key::Name(");
                n.emit(out);
                out.push(')');
            }
            Key::Local(i) => {
                let _ = write!(out, "Key::Local({i})");
            }
        }
    }
}

/// Enums of the shape `Type::Catalog(n) | Type::Blueprint(n)`.
macro_rules! emit_ref {
    ($($t:ident),*) => {$(
        impl Emit for $t {
            fn emit(&self, out: &mut String) {
                let _ = match self {
                    $t::Catalog(i) => write!(out, concat!(stringify!($t), "::Catalog({})"), i),
                    $t::Blueprint(i) => write!(out, concat!(stringify!($t), "::Blueprint({})"), i),
                };
            }
        }
    )*};
}
emit_ref!(ListRef, CondsRef, NamesRef);

impl Emit for TextRef {
    fn emit(&self, out: &mut String) {
        match self {
            TextRef::Text(t) => {
                out.push_str("TextRef::Text(");
                t.emit(out);
                out.push(')');
            }
            TextRef::Local(i) => {
                let _ = write!(out, "TextRef::Local({i})");
            }
        }
    }
}

impl Emit for Stack {
    fn emit(&self, out: &mut String) {
        out.push_str("Stack { item: ");
        self.item.emit(out);
        let _ = write!(out, ", qty: {} }}", self.qty);
    }
}

impl Emit for FlagKey {
    fn emit(&self, out: &mut String) {
        let (v, k) = match self {
            FlagKey::Named(k) => ("Named", k),
            FlagKey::Been(k) => ("Been", k),
            FlagKey::Dead(k) => ("Dead", k),
        };
        let _ = write!(out, "FlagKey::{v}(");
        k.emit(out);
        out.push(')');
    }
}

impl Emit for FlagOp {
    fn emit(&self, out: &mut String) {
        let _ = match self {
            FlagOp::Set(v) => write!(out, "FlagOp::Set({v})"),
            FlagOp::Add(v) => write!(out, "FlagOp::Add({v})"),
        };
    }
}

impl Emit for FlagTest {
    fn emit(&self, out: &mut String) {
        let _ = match self {
            FlagTest::Eq(v) => write!(out, "FlagTest::Eq({v})"),
            FlagTest::Min(v) => write!(out, "FlagTest::Min({v})"),
            FlagTest::NonZero => write!(out, "FlagTest::NonZero"),
        };
    }
}

impl Emit for Heal {
    fn emit(&self, out: &mut String) {
        match self {
            Heal::Flat(m) => {
                out.push_str("Heal::Flat(");
                m.emit(out);
            }
            Heal::Pct(p) => {
                out.push_str("Heal::Pct(");
                p.emit(out);
            }
        }
        out.push(')');
    }
}

impl Emit for Thing {
    fn emit(&self, out: &mut String) {
        match self {
            Thing::Item(i) => {
                out.push_str("Thing::Item(");
                i.emit(out);
            }
            Thing::Prop(p) => {
                out.push_str("Thing::Prop(");
                p.emit(out);
            }
        }
        out.push(')');
    }
}

impl Emit for FactKey {
    fn emit(&self, out: &mut String) {
        let one = |out: &mut String, v: &str, e: &dyn Fn(&mut String)| {
            let _ = write!(out, "FactKey::{v}(");
            e(out);
            out.push(')');
        };
        match self {
            FactKey::Place(k) => one(out, "Place", &|o| k.emit(o)),
            FactKey::Person(k) => one(out, "Person", &|o| k.emit(o)),
            FactKey::Thing(t) => one(out, "Thing", &|o| t.emit(o)),
            FactKey::Claim(t) => one(out, "Claim", &|o| t.emit(o)),
            FactKey::Danger(k) => one(out, "Danger", &|o| k.emit(o)),
            FactKey::Rumour(s) => one(out, "Rumour", &|o| s.emit(o)),
            FactKey::Route(a, b) => one(out, "Route", &|o| {
                a.emit(o);
                o.push_str(", ");
                b.emit(o);
            }),
        }
    }
}

impl Emit for Condition {
    fn emit(&self, out: &mut String) {
        let wrap = |out: &mut String, v: &str, e: &dyn Fn(&mut String)| {
            let _ = write!(out, "Condition::{v}(");
            e(out);
            out.push(')');
        };
        match self {
            Condition::Flag { key, test } => {
                out.push_str("Condition::Flag { key: ");
                key.emit(out);
                out.push_str(", test: ");
                test.emit(out);
                out.push_str(" }");
            }
            Condition::Night => out.push_str("Condition::Night"),
            Condition::QuestActive(q) => wrap(out, "QuestActive", &|o| q.emit(o)),
            Condition::QuestReady(q) => wrap(out, "QuestReady", &|o| q.emit(o)),
            Condition::QuestDone(q) => wrap(out, "QuestDone", &|o| q.emit(o)),
            Condition::HasItem(s) => wrap(out, "HasItem", &|o| s.emit(o)),
            Condition::HasSpell(s) => wrap(out, "HasSpell", &|o| s.emit(o)),
            Condition::Dead(k) => wrap(out, "Dead", &|o| k.emit(o)),
            Condition::Knows(f) => wrap(out, "Knows", &|o| f.emit(o)),
            Condition::Heard(t) => wrap(out, "Heard", &|o| t.emit(o)),
            Condition::SpeakerKnows(s) => wrap(out, "SpeakerKnows", &|o| s.emit(o)),
        }
    }
}

impl Emit for Cond {
    fn emit(&self, out: &mut String) {
        let _ = write!(out, "Cond {{ not: {}, c: ", self.not);
        self.c.emit(out);
        out.push_str(" }");
    }
}

/// Writes `Name { a: .., b: .. }` for struct-like enum variants and structs.
struct Fields<'a> {
    out: &'a mut String,
    first: bool,
}

impl<'a> Fields<'a> {
    fn open(out: &'a mut String, name: &str) -> Self {
        out.push_str(name);
        out.push_str(" { ");
        Self { out, first: true }
    }

    fn f(mut self, name: &str, v: &dyn EmitDyn) -> Self {
        if !self.first {
            self.out.push_str(", ");
        }
        self.first = false;
        self.out.push_str(name);
        self.out.push_str(": ");
        v.emit_dyn(self.out);
        self
    }

    fn close(self) {
        self.out.push_str(" }");
    }
}

/// Object-safe `Emit`, for the field writer.
pub trait EmitDyn {
    fn emit_dyn(&self, out: &mut String);
}

impl<T: Emit + ?Sized> EmitDyn for T {
    fn emit_dyn(&self, out: &mut String) {
        self.emit(out);
    }
}

impl Emit for Action {
    fn emit(&self, out: &mut String) {
        let one = |out: &mut String, v: &str, e: &dyn EmitDyn| {
            let _ = write!(out, "Action::{v}(");
            e.emit_dyn(out);
            out.push(')');
        };
        match self {
            Action::Quest(q) => one(out, "Quest", q),
            Action::HandIn(q) => one(out, "HandIn", q),
            Action::Flag { key, op } => Fields::open(out, "Action::Flag").f("key", key).f("op", op).close(),
            Action::Rest { until } => Fields::open(out, "Action::Rest").f("until", until).close(),
            Action::Grow { stat, amount, id } => {
                Fields::open(out, "Action::Grow").f("stat", stat).f("amount", amount).f("id", id).close();
            }
            Action::Give(s) => one(out, "Give", s),
            Action::Take(s) => one(out, "Take", s),
            Action::Learn(s) => one(out, "Learn", s),
            Action::Toast(t) => one(out, "Toast", t),
            Action::Read(t) => one(out, "Read", t),
            Action::Lock(k) => one(out, "Lock", k),
            Action::Unlock(k) => one(out, "Unlock", k),
            Action::Show(k) => one(out, "Show", k),
            Action::Hide(k) => one(out, "Hide", k),
            Action::Switch { prop, on } => Fields::open(out, "Action::Switch").f("prop", prop).f("on", on).close(),
            Action::Spawn { key, def, at } => {
                Fields::open(out, "Action::Spawn").f("key", key).f("def", def).f("at", at).close();
            }
            Action::Despawn(k) => one(out, "Despawn", k),
            Action::Aggro(k) => one(out, "Aggro", k),
            Action::Location(k) => one(out, "Location", k),
            Action::Fill { rect, tile } => Fields::open(out, "Action::Fill").f("rect", rect).f("tile", tile).close(),
            Action::Strike { rect, amount, school, effect, hits_friends } => Fields::open(out, "Action::Strike")
                .f("rect", rect)
                .f("amount", amount)
                .f("school", school)
                .f("effect", effect)
                .f("hits_friends", hits_friends)
                .close(),
            Action::Status(e) => one(out, "Status", e),
            Action::Heal(h) => one(out, "Heal", h),
            Action::Travel { zone, mark } => {
                Fields::open(out, "Action::Travel").f("zone", zone).f("mark", mark).close()
            }
            Action::Talk(d) => one(out, "Talk", d),
            Action::Throw(i) => one(out, "Throw", i),
            Action::Shake(n) => one(out, "Shake", n),
            Action::Camera { mode, rect } => {
                Fields::open(out, "Action::Camera").f("mode", mode).f("rect", rect).close()
            }
            Action::If { when, then, els } => {
                Fields::open(out, "Action::If").f("when", when).f("then", then).f("els", els).close();
            }
            Action::Send { unit, to, then } => {
                Fields::open(out, "Action::Send").f("unit", unit).f("to", to).f("then", then).close();
            }
            Action::Reveal(n) => one(out, "Reveal", n),
        }
    }
}

/// Define a model struct and its emitter together, so no field can be left out of either.
///
/// ```ignore
/// model! {
///     /// An item.
///     pub struct ItemDef {
///         pub id: &'static str,
///         pub max_stack: u16,
///     }
/// }
/// ```
#[macro_export]
macro_rules! model {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }

        impl $crate::emit::Emit for $name {
            #[allow(unused_variables)]
            fn emit(&self, out: &mut String) {
                out.push_str(concat!(stringify!($name), " {"));
                $(
                    out.push_str(concat!(" ", stringify!($f), ": "));
                    $crate::emit::Emit::emit(&self.$f, out);
                    out.push(',');
                )*
                out.push_str(" }");
            }
        }
    };
}

/// Define a fieldless model enum and its emitter together.
#[macro_export]
macro_rules! model_enum {
    ($(#[$m:meta])* pub enum $name:ident { $($(#[$vm:meta])* $v:ident),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name { $($(#[$vm])* $v),* }

        impl $crate::emit::Emit for $name {
            fn emit(&self, out: &mut String) {
                out.push_str(match self { $($name::$v => concat!(stringify!($name), "::", stringify!($v))),* });
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_and_containers() {
        assert_eq!(to_rust(&Some(3u16)), "Some(3)");
        assert_eq!(to_rust(&"a\"b"), "\"a\\\"b\"");
        let v: &[u8] = &[1, 2];
        assert_eq!(to_rust(v), "&[1, 2]");
        assert_eq!(to_rust(&ItemId(4)), "ItemId(4)");
        assert_eq!(to_rust(&ZoneId::Mine), "ZoneId::Mine");
        assert_eq!(to_rust(&Tile::CaveFloor), "Tile::CaveFloor");
    }

    #[test]
    fn actions_emit_as_rust() {
        let a = Action::Strike {
            rect: Key::Name(NameId(2)),
            amount: Milli(5000),
            school: School::Fire,
            effect: None,
            hits_friends: false,
        };
        assert_eq!(
            to_rust(&a),
            "Action::Strike { rect: Key::Name(NameId(2)), amount: Milli(5000), school: School::Fire, effect: None, hits_friends: false }"
        );
        assert_eq!(to_rust(&Action::Shake(3)), "Action::Shake(3)");
        let c = Cond { not: true, c: Condition::Knows(FactKey::Route(Key::Local(1), Key::Name(NameId(0)))) };
        assert_eq!(
            to_rust(&c),
            "Cond { not: true, c: Condition::Knows(FactKey::Route(Key::Local(1), Key::Name(NameId(0)))) }"
        );
    }
}
