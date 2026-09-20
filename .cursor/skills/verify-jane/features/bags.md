# Bags

One tabbed DOM window: Inventory, Spellbook, Quests, Map (`.jq-tab`). I or Tab opens Inventory, K the Spellbook, J Quests, M the Map; the same key closes it; A / D or Q cycle tabs; Esc closes. The world is paused while it is open and the action bar stays live for drag and drop.

## Sub-features

- `bags-open` I opens Inventory with 24 slots and a stats pane (Health, Mana, Strength 30, Spirit 30, Kills, Falls).
- `bags-tooltip` hovering the apple shows its name, description and cooldown.
- `bags-move` dragging a stack to another slot moves it; onto a bar slot binds it.
- `bags-use` right-click or double-click an apple below full health eats it (+25% health).
- `bags-destroy-bound` dragging Julie's Letter out of the window is refused ("I should keep that").
- `bags-craft-gated` away from a bench the craft row is disabled with "Stand at a bench to craft".
- `bags-craft` in Julie's kitchen, standing at the bench: open the pantry chest first, drag Gold Dust, Small Vial of Water and Pansy into the three inputs; the output shows Small Manashield Potion; clicking it consumes the inputs.
- `bags-book` Spellbook lists Melee; after touching the ice orb, Icebolt with cost 14, range 15 m, cooldown 2 s.
- `bags-quests` Quests shows "A Letter from Julie" with its requirement and count.
- `bags-map` Map draws the zone; interiors only show what has been seen.

## Recipe

Drive at least `bags-open`, `bags-move`, `bags-craft-gated` and `bags-craft` with real pointer drags. Record the bag before and after from `jane.sim().player.bag`. Pointer drags were not exercised in a browser when the UI was written, so this recipe is the first real proof of them.
