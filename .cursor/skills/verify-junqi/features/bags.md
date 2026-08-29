# Bags

The bag sheet is a Phaser window. I or Tab opens it. Tabs read Bag, Book, Quest, Map. A new run shows Julie's letter, apples, and the birthday present.

## Sub-features

- `bags-open-i` opens the sheet from I.
- `bags-open-tab` opens the sheet from Tab.
- `bags-tabs` draws Bag, Book, Quest, Map.
- `bags-stats` draws Strength 140 · Spirit 140.
- `bags-start-kit` shows the starting letter and apples (console `status` lists the ids).

## How to get to it (user POV)

- Press I in play.
- Press Tab in play.
- Click the Bag / Book / Quest / Map tabs on the sheet.

## Driving it with the browser

Preconditions:

- Doctor reports `ok: true`.
- A new run is on county. Console is **closed**.

- **Open I.** Press I. Screenshot `evidence/bags/bag.png`. Tabs read Bag Book Quest Map. Stats line reads Strength 140 and Spirit 140.
- **Ids.** Open the console and run `status`. Log contains `julies_letter`, `apple`, `birthday_present`. Canvas icons have no ARIA names — the log is the id proof; the screenshot is the sheet proof.
- **Tab entry.** Close console and bags (I or Tab again). Press Tab. Same sheet. Arrow keys cycle windows; proving Book needs a second shot that is not the Bag tab.
- **Proof.** `evidence/bags/bag.png` plus `evidence/bags/console.txt` from `status`.

## Gotchas

- I typed into `#term` is not the bag key. Close the console first.
- Snapshot will not list a Bag button.
- Craft slots only appear near the kitchen bench (`near_bench`). Do not require them on the stoop.
- Esc pauses the world and does not close bags the same way I does. Prefer I/Tab to toggle the sheet.
