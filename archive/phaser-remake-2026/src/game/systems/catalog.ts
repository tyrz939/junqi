import items from "@/data/items.json";
import spells from "@/data/spells.json";
import spellsMore from "@/data/spells-more.json";
import recipes from "@/data/recipes.json";
import beats from "@/data/beats.json";
import enemies from "@/data/enemies.json";
import enemiesMore from "@/data/enemies-more.json";
import quests from "@/data/quests.json";
import questsMore from "@/data/quests-more.json";
import effects from "@/data/effects.json";
import type { BeatDef, EffectDef, EnemyDef, ItemDef, QuestDef, RecipeDef, SpellDef } from "@/game/types";

export const itemCatalog = items as Record<string, ItemDef>;
export const spellCatalog = { ...(spells as Record<string, SpellDef>), ...(spellsMore as Record<string, SpellDef>) };
export const recipeCatalog = recipes as RecipeDef[];
export const beatSpine = (beats as { spine: BeatDef[] }).spine;
export const enemyCatalog = { ...(enemies as Record<string, EnemyDef>), ...(enemiesMore as Record<string, EnemyDef>) };
export const questCatalog = { ...(quests as Record<string, QuestDef>), ...(questsMore as Record<string, QuestDef>) };
export const effectCatalog = effects as Record<string, EffectDef>;

export function getQuest(id: string): QuestDef {
  const quest = questCatalog[id];
  if (!quest) throw new Error(`Unknown quest: ${id}`);
  return quest;
}

export function getItem(id: string): ItemDef {
  const item = itemCatalog[id];
  if (!item) throw new Error(`Unknown item: ${id}`);
  return item;
}

export function getSpell(id: string): SpellDef {
  const spell = spellCatalog[id];
  if (!spell) throw new Error(`Unknown spell: ${id}`);
  return spell;
}

export function getEffect(id: string): EffectDef {
  const effect = effectCatalog[id];
  if (!effect) throw new Error(`Unknown effect: ${id}`);
  return effect;
}

export function getEnemy(id: string): EnemyDef {
  const enemy = enemyCatalog[id];
  if (!enemy) throw new Error(`Unknown enemy: ${id}`);
  return enemy;
}

export function keyForLock(opens: string): string {
  for (const [id, item] of Object.entries(itemCatalog)) {
    if (item.opens === opens) return id;
  }
  return opens;
}

export function matchRecipe(inputIds: string[]): RecipeDef | undefined {
  const want = [...inputIds].filter(Boolean).sort();
  return recipeCatalog.find((recipe) => {
    const have = [...recipe.inputs].sort();
    return have.length === want.length && have.every((id, i) => id === want[i]);
  });
}
