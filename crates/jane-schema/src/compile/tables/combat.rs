//! Compile the `combat` group into [`model::Combat`].

use serde::Deserialize;

use jane_core::ids::ItemId;

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::fraction::Num;
use crate::compile::lists::{self, RawAction};
use crate::compile::source::{Source, typed};
use crate::model::{self, ItemDef, RecipeDef};

pub fn compile(src: &Source, cx: &mut Ctx) -> model::Combat {
    model::Combat { items: items(src, cx), recipes: recipes(src, cx) }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawItem {
    name: String,
    description: String,
    icon: String,
    max_stack: u16,
    usable: bool,
    cooldown: Num,
    #[serde(rename = "use")]
    use_list: Option<Vec<RawAction>>,
    #[serde(default)]
    keep: bool,
    opens: Option<String>,
    #[serde(default)]
    bound: bool,
}

/// Items a quest asks the party to acquire, read from the quests' raw rows: they are story items.
fn acquired(src: &Source, cx: &mut Ctx) -> Vec<String> {
    let mut quiet = crate::compile::diag::Diagnostics::default();
    let mut out = Vec::new();
    for row in src.table("quests", &mut quiet).values() {
        for req in row.value.get("requirements").and_then(|r| r.as_array()).into_iter().flatten() {
            if req.get("type").and_then(|t| t.as_str()) == Some("acquire") {
                if let Some(t) = req.get("target").and_then(|t| t.as_str()) {
                    out.push(t.to_owned());
                }
            }
        }
    }
    let _ = cx;
    out
}

fn items(src: &Source, cx: &mut Ctx) -> &'static [ItemDef] {
    let rows = src.table("items", &mut cx.diag);
    let acquired = acquired(src, cx);
    let mut out: Vec<Option<ItemDef>> = vec![None; cx.ids.items.len()];
    for (id, row) in &rows {
        let at = format!("items.{id}");
        let Some(r) = typed::<RawItem>(row, &at, &mut cx.diag) else { continue };
        let cooldown = r.cooldown.ticks().map_err(|e| cx.diag.error(&at, e)).unwrap_or_default();
        cx.diag.need(r.max_stack >= 1, &at, "maxStack < 1");
        cx.diag.need(
            !r.usable || r.use_list.is_some() || r.opens.is_some(),
            &at,
            "usable but has neither use[] nor opens",
        );
        let def = ItemDef {
            id: leak_str(id),
            name: cx.text(&r.name),
            description: cx.text(&r.description),
            icon: cx.sprite(&r.icon),
            max_stack: r.max_stack,
            usable: r.usable,
            cooldown,
            use_list: lists::list(cx, &format!("{at}.use"), r.use_list.as_deref()),
            keep: r.keep,
            opens: r.opens.as_deref().map(|o| cx.name(o)),
            bound: r.bound,
            story: r.opens.is_some() || acquired.iter().any(|a| a == id),
        };
        let i = cx.ids.items.get(id).map_or(0, usize::from);
        out[i] = Some(def);
    }
    // A row that failed to type leaves a hole; the diagnostics already say why, and no catalog
    // is produced, so the hole is never read.
    leak(out.into_iter().map(|d| d.unwrap_or_else(placeholder_item)).collect())
}

fn placeholder_item() -> ItemDef {
    ItemDef {
        id: "",
        name: jane_core::TextId(0),
        description: jane_core::TextId(0),
        icon: jane_core::SpriteId(0),
        max_stack: 1,
        usable: false,
        cooldown: jane_core::Tick(0),
        use_list: None,
        keep: false,
        opens: None,
        bound: false,
        story: false,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecipe {
    inputs: Vec<String>,
    output: String,
    qty: u16,
}

fn recipes(src: &Source, cx: &mut Ctx) -> &'static [RecipeDef] {
    let rows = src.list("recipes", &mut cx.diag);
    let mut out: Vec<RecipeDef> = Vec::new();
    for (n, row) in rows.iter().enumerate() {
        let at = format!("recipes[{n}]");
        let Some(r) = typed::<RawRecipe>(row, &at, &mut cx.diag) else { continue };
        cx.diag.need((1..=3).contains(&r.inputs.len()), &at, "1..3 inputs");
        cx.diag.need(r.qty >= 1, &at, "qty < 1");
        let mut inputs: Vec<ItemId> = r.inputs.iter().filter_map(|i| cx.item(&at, i)).collect();
        inputs.sort();
        let Some(output) = cx.item(&at, &r.output) else { continue };
        if out.iter().any(|o| o.inputs == inputs.as_slice()) {
            cx.diag.error(&at, format!("duplicate recipe {}", r.inputs.join("+")));
        }
        out.push(RecipeDef { inputs: leak(inputs), output, qty: r.qty });
    }
    leak(out)
}
