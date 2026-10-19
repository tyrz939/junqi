#!/usr/bin/env python3
"""Walk tests for the PSP build on PPSSPP (the PSP v0.4.1 plan, WP1).

    python -I tools/psp-walk-test.py <dir-with-prx-and-assets> [--case calm|heavy|heavy-nopaint|squeeze|listcap|run|run-town|all]

<dir> holds psp-game.prx, present.jpt, jane-psp.jpk, jane-psp.jau and county.bp (the build and its bake;
tools/psp-dist.sh leaves them under target/psp-dist and spikes/psp-game/target). Each case writes a
script.txt into <dir>, runs PPSSPPHeadless on it as a PSP-1000 (PPSSPP_PSP1000=1, PORT.md 13.8)
with its own memory stick in <dir>/memstick (so perf.txt, crash.txt and the captures are this run's
alone), and reads what the game printed. The county is baked (county.bp beside the program), never built:

  GAME pace ...    a window's flips, missed vblanks, swatches, boosts, list guard cuts, paint fails
  GAME jobs ...    the painter's jobs (wall_us a job)
  GAME small ...   list_most and the patch buffers (lister, GE) as (count, bytes)
  GAME her ...     where she is (the walk must take her three chunks or more)
  GAME scene=Play  the window's fps, batches and free RAM

and the dashcam the script presses (L + R + START) a few seconds before its end: ticks a frame,
vblanks a flip, batches. It prints one table and exits non-zero when a case fails.

Cases (all seed 570944055, perf overlay on, tpf:2 = a steady 30 locked):
  calm           the station road at 940,1500 (the first walk) at 14:13, 1800 ticks north-west along it
                 toward Julie's (if that is blocked: west for a third, then north)
  heavy          the road into Castle at 760,1130 at 17:22, north into the town 900 ticks
  heavy-nopaint  heavy with the painter handed no job: swatches forced
  squeeze        heavy-nopaint 1800 ticks with the heap squeezed to 2 MB free (ballast:auto2048)
  listcap        heavy-nopaint with the display list's cap at 48 KB: the list guard must act (the
                 flat swatches' list is 76 KB; at 96 KB it no longer does, since WP2 took the domes
                 from the stand-ins that once filled 250 KB) and the list must stay under it
  run            R held (she sprints while her energy lasts, walks while it fills, sprints again)
                 from 940,1500 up the station road, past Julie's and up the road to Castle's edge,
                 3000 ticks, with slowpaint:330 (each paint job holds the painter 3.3 times as
                 long: the real PSP-1000 lands about 1.1 chunks a second, PPSSPP about 3.5)
  run-town       the same through Castle: its main street east from 716,999, then south-east out,
                 2100 ticks

The runs print the ground's demand and supply (her pace, the chunks coming on screen a second,
the painter's landings a second) and are judged on the frames whose stand-ins show mostly flat
colour (mush_frames: under 1%, never 2 s in a row) and free RAM (1.5 MB or more). `--slowpaint`
puts the handicap on any case, `--word noperf` takes the overlay off (for screenshots: the runs
shoot shot-p<tick>.bmp), `--legs` walks other legs.

The thresholds are the plan's. The ones for the paint (swatches) and the list (batches, list_most,
patches, list_full) hold only once WP2 and WP3 are in: they are checked on any build and shown
as the build's own numbers either way. PPSSPP does not time the GE's fill, so the vblanks and fps
here are the CPU side's alone (indicative only; the hardware dashcam is the truth). A run that
dies on a CPU fault (a bad load or store: no Rust panic, so no crash.txt) shows PPSSPP's own
report of it and the last window the game printed.
"""

import argparse
import os
import re
import shutil
import struct
import subprocess
import sys
import time
from pathlib import Path

PPSSPP = r"C:\Users\kille\tools\ppsspp-src\Windows\x64\Release\PPSSPPHeadless.exe"
SEED = 570944055
# A chunk is 256 px of 16-px cells.
CHUNK_CELLS = 16
# The first ticks hold the travel's view and the first paints: judged after them.
SETTLE_TICKS = 120
# The dashcam is pressed this many ticks before the end (it saves five seconds later).
CAPTURE_LEAD = 420
SAVE = Path("memstick") / "PSP" / "SAVEDATA" / "JANE00001"

# name -> (stand cell, clock, ticks, extra words)
CASES = {
    "calm": ((940, 1500), "14:13", 1800, []),
    "heavy": ((760, 1130), "17:22", 900, []),
    "heavy-nopaint": ((760, 1130), "17:22", 900, ["nopaint"]),
    "squeeze": ((760, 1130), "17:22", 1800, ["nopaint", "ballast:auto2048"]),
    "listcap": ((760, 1130), "17:22", 900, ["nopaint", "listcap:48"]),
    # R held throughout (sprint: she runs while her energy lasts, walks while it fills, and runs
    # again), up the station road and on north through Castle: 70 s of play.
    "run": ((940, 1500), "14:13", 3000, ["hold:r@p60-p3000", "slowpaint:330"]),
    "run-town": ((716, 999), "17:22", 2100, ["hold:r@p60-p2100", "slowpaint:330"]),
}
# The runs' legs (degrees, from tick, to tick): north-west up the station road, north past Julie's
# plot, west a little onto the road north, and up it to the edge of Castle; the town's run east
# along its main street, then south-east out of it.
RUN_LEGS = {
    "run": [(225, 60, 1150), (270, 1150, 1400), (180, 1400, 1600), (270, 1600, 3000)],
    "run-town": [(0, 60, 1100), (45, 1100, 2100)],
}
# Shots the runs take (shot-p<tick>.bmp beside the program): moments she is running.
RUN_SHOTS = (300, 700, 1100, 1700, 2300)


def script_text(case, legs=None, capture=True, more=()):
    """The script for `case`: its words, the legs of the walk (degrees, from tick, to tick) and the dashcam's press."""
    (x, y), clock, ticks, extra = CASES[case]
    extra = list(extra) + list(more)
    legs = legs or [(0, 60, ticks)]
    words = [f"seed:{SEED}", "perf", "tpf:2", f"stand:{x},{y}", f"clock:{clock}"] + extra
    if "noperf" in words:
        words = [w for w in words if w not in ("perf", "noperf")]
    words += [f"stick:{d}@p{a}-p{b}" for d, a, b in legs]
    if case in RUN_LEGS:
        words += [f"shot@p{t}" for t in RUN_SHOTS if t < ticks]
    if capture:
        c = max(SETTLE_TICKS, ticks - CAPTURE_LEAD)
        words += [f"hold:l@p{c}-p{c + 10}", f"hold:r@p{c}-p{c + 10}", f"hold:start@p{c + 3}-p{c + 10}"]
    words.append(str(ticks))
    return " ".join(words)


def kv(line):
    return dict(re.findall(r"(\w+)=(\S+)", line))


def parse_log(text):
    """The log's windows: each `GAME scene=` line begins one, the lines after it join it."""
    windows, cur = [], None
    out = {"windows": windows, "done": None, "panic": None, "timeout": "TIMEOUT" in text, "errors": []}
    for line in text.splitlines():
        if not line.startswith("GAME "):
            continue
        if line.startswith("GAME scene="):
            f = kv(line)
            cur = {"scene": f.get("scene"), "fps": f.get("fps"), "batches": int(f.get("batches", 0)),
                   "free": int(f.get("free", 0)), "maxfree": int(f.get("maxfree", 0))}
            windows.append(cur)
        elif cur is not None and line.startswith("GAME pace "):
            cur["pace"] = {k: int(v) if v.lstrip("-").isdigit() else v for k, v in kv(line).items()}
        elif cur is not None and line.startswith("GAME small "):
            f = kv(line)
            cur["list_most"] = int(f.get("list_most", 0))
            m = re.search(r"patches=\((\d+), (\d+)\) \((\d+), (\d+)\)", line)
            if m:
                cur["patches"] = tuple(int(g) for g in m.groups())
        elif cur is not None and line.startswith("GAME baked landed="):
            cur["baked"] = {k: int(v) for k, v in kv(line).items()}
        elif cur is not None and line.startswith("GAME jobs "):
            cur["jobs"] = {k: int(v) for k, v in kv(line).items()}
        elif cur is not None and line.startswith("GAME her "):
            m = re.search(r"cell=\((-?\d+), (-?\d+)\)", line)
            if m:
                cur["cell"] = (int(m.group(1)), int(m.group(2)))
        elif line.startswith("GAME done "):
            out["done"] = kv(line)
        elif line.startswith("GAME panic"):
            out["panic"] = line
        elif line.startswith("GAME error"):
            out["errors"].append(line)
    return out


def cpu_fault(text):
    """A CPU fault PPSSPP stopped the run on (a bad load or store: no Rust panic, so no crash.txt), with its call stack."""
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if "SIGSEGV" in line or "Bad memory" in line:
            block = [line]
            for more in lines[i + 1:i + 8]:
                if not more.strip() or more.startswith(("GAME ", "AUDIO ")):
                    break
                block.append(more)
            return "\n".join(l[:260] for l in block)
    return None


def read_capture(path):
    """A dashcam capture (crates/jane-render-psp/src/capture.rs): its header and each frame's fields by name."""
    b = path.read_bytes()
    if b[:4] != b"JCAP":
        raise ValueError("not a capture")
    (hn,) = struct.unpack_from("<I", b, 8)
    header = dict(l.split(" ", 1) if " " in l else (l, "") for l in b[12:12 + hn].decode().splitlines() if l)
    at = 12 + hn
    nf, npass = struct.unpack_from("<HH", b, at)
    at += 4
    names = []
    for _ in range(nf + npass):
        n = b[at]
        names.append(b[at + 1:at + 1 + n].decode())
        at += 1 + n
    (frames,) = struct.unpack_from("<I", b, at)
    at += 4
    words = nf + npass
    data = struct.unpack_from(f"<{frames * words}I", b, at)
    cols = {name: [data[f * words + i] for f in range(frames)] for i, name in enumerate(names[:nf])}
    return header, cols


def p95(xs):
    xs = sorted(xs)
    return xs[(len(xs) - 1) * 95 // 100] if xs else 0


def hist(xs):
    h = {}
    for x in xs:
        h[x] = h.get(x, 0) + 1
    return dict(sorted(h.items()))


def run_ppsspp(exe, folder, case, wall_timeout, emu_timeout):
    """One run of the script now in `folder`: its stdout, whether it timed out on the wall clock, the seconds it took."""
    env = dict(os.environ, PPSSPP_PSP1000="1")
    cmd = [exe, str(folder / "psp-game.prx"), f"--root={folder}", f"--memstick={folder / 'memstick'}", "-j",
           f"--timeout={emu_timeout}"]
    t = time.time()
    try:
        p = subprocess.run(cmd, env=env, capture_output=True, text=True, errors="replace", timeout=wall_timeout)
        text, wall_out = p.stdout + p.stderr, False
    except subprocess.TimeoutExpired as e:
        text = (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or "")
        wall_out = True
    (folder / f"{case}.log").write_text(text, encoding="utf-8")
    return text, wall_out, time.time() - t


def analyse(case, folder, text, wall_out, secs, legs):
    """Everything a case is judged on, from the log, the stick's files and the dashcam."""
    log = parse_log(text)
    save = folder / SAVE
    r = {"case": case, "secs": secs, "legs": legs, "checks": [], "notes": []}
    play = [w for w in log["windows"] if w["scene"] == "Play" and "pace" in w]
    r["play_windows"] = len(play)
    crash = save / "crash.txt"
    r["crash"] = crash.read_text(errors="replace") if crash.exists() else None
    r["panic"] = log["panic"]
    r["fault"] = cpu_fault(text)
    r["done"] = log["done"] is not None
    r["wall_out"] = wall_out or log["timeout"]
    # The last window printed before the run ended (where a fault or a timeout hit, give or take two seconds).
    last = play[-1] if play else None
    r["last_tick"] = last["pace"]["tick"] if last else 0
    r["last_pace"] = (
        f"list_most {last.get('list_most', 0) // 1024} KB, free {last['free'] // 1024} KB,"
        f" largest {last['maxfree'] // 1024} KB, most swatches waiting {last['pace']['wait_max']}"
        if last
        else ""
    )
    # Where she went: the first and last cell printed in play.
    cells = [w["cell"] for w in play if "cell" in w]
    r["moved"] = max(abs(cells[-1][0] - cells[0][0]), abs(cells[-1][1] - cells[0][1])) if len(cells) > 1 else 0
    # The windows after the settle: their start tick is past it.
    late = [w for w in play if w["pace"]["tick"] - w["pace"]["ticks"] >= SETTLE_TICKS]
    flips = sum(w["pace"]["flips"] for w in late)
    missed = sum(w["pace"]["missed"] for w in late)
    r["flips"], r["missed"] = flips, missed
    r["missed_pct"] = missed * 100.0 / flips if flips else 0.0
    r["flip_max"] = max([w["pace"]["flip_max"] for w in late], default=0)
    r["swatches"] = sum(w["pace"]["swatches"] for w in late)
    r["swatches_all"] = sum(w["pace"]["swatches"] for w in play)
    # The baked county's loader (P1c): its counts are running totals; stand-ins on screen after the settle.
    baked = [w["baked"] for w in play if "baked" in w]
    settled = [w["baked"] for w in late if "baked" in w]
    r["baked"] = baked[-1] if baked else None
    r["stand_ins_late"] = (baked[-1]["stand_ins_shown"] - settled[0]["stand_ins_shown"]) if settled else None
    r["wait_frames"] = sum(w["pace"]["wait_frames"] for w in late)
    r["wait_max"] = max([w["pace"]["wait_max"] for w in late], default=0)
    r["boosts"] = sum(w["pace"]["boosts"] for w in play)
    r["extra_waits"] = sum(w["pace"]["extra_waits"] for w in play)
    r["list_full"] = sum(w["pace"]["list_full"] for w in play)
    # What the lister left out for want of room (a build before WP3 prints none of these).
    r["left_out"] = tuple(
        sum(w["pace"].get(k, 0) for w in play) for k in ("quads_dropped", "polys_skipped", "patches_skipped")
    )
    r["paint_fail"] = max([w["pace"]["paint_fail"] for w in play], default=0)
    r["list_most"] = max([w.get("list_most", 0) for w in play], default=0)
    settled = [w for w in play if w["pace"]["tick"] >= 300]
    patches = [w["patches"] for w in settled if "patches" in w]
    r["patches_first"], r["patches_last"] = (patches[0], patches[-1]) if patches else (None, None)
    r["patches_flat"] = len(set(patches)) <= 1
    frees = [w["free"] for w in settled]
    r["free_min"], r["free_max"] = (min(frees), max(frees)) if frees else (0, 0)
    r["maxfree_min"] = min([w["maxfree"] for w in settled], default=0)
    jobs = [w["jobs"]["wall_us"] for w in play if "jobs" in w and w["jobs"].get("n", 0) > 0]
    r["job_wall_us"] = jobs[-1] if jobs else 0
    # The ground's demand and supply after the settle: her pace, the chunks coming on screen a
    # second (the view's 30 x 17 cells round her, her path between the windows' samples taken
    # straight), the painter's landings a second, the share of frames with swatches showing and
    # the longest run of such frames (`wait_streak`, a build that prints it).
    track = [(w["pace"]["tick"], w["cell"], w.get("jobs", {})) for w in late if "cell" in w]
    r["cells_per_s"] = r["chunks_per_s"] = r["landed_per_s"] = 0.0
    if len(track) > 1:
        secs = (track[-1][0] - track[0][0]) / 60.0
        path, seen = 0.0, set()
        for (_, (x0, y0), _), (_, (x1, y1), _) in zip(track, track[1:]):
            path += ((x1 - x0) ** 2 + (y1 - y0) ** 2) ** 0.5
            n = max(abs(x1 - x0), abs(y1 - y0), 1)
            for k in range(n + 1):
                x, y = x0 + (x1 - x0) * k // n, y0 + (y1 - y0) * k // n
                for cy in range((y - 9) // CHUNK_CELLS, (y + 9) // CHUNK_CELLS + 1):
                    for cx in range((x - 15) // CHUNK_CELLS, (x + 15) // CHUNK_CELLS + 1):
                        seen.add((cx, cy))
        r["cells_per_s"] = path / secs
        r["chunks_per_s"] = len(seen) / secs
        landed = [j["landed"] for _, _, j in track if "landed" in j]
        if len(landed) > 1:
            r["landed_per_s"] = (landed[-1] - landed[0]) / secs
    r["wait_pct"] = r["wait_frames"] * 100.0 / flips if flips else 0.0
    r["wait_streak"] = max([w["pace"].get("wait_streak", 0) for w in play], default=0)
    # Of those, the frames a stand-in on screen is mostly flat colour (no sample of its tiles yet):
    # the blocks of colour (a build that prints them; else every swatch frame counts).
    has_mush = any("mush_frames" in w["pace"] for w in play)
    r["mush_frames"] = sum(w["pace"].get("mush_frames", 0) for w in late) if has_mush else r["wait_frames"]
    r["mush_streak"] = max([w["pace"].get("mush_streak", 0) for w in play], default=0) if has_mush else r["wait_streak"]
    r["mush_pct"] = r["mush_frames"] * 100.0 / flips if flips else 0.0
    # The dashcam.
    r["cap"] = None
    caps = sorted(save.glob("capture-*.bin")) if save.exists() else []
    if caps:
        try:
            header, cols = read_capture(caps[-1])
            n = len(cols["frame"])
            r["cap"] = {
                "frames": n,
                "pacing": header.get("pacing", "?"),
                "n_ticks": hist(cols["n_ticks"]),
                "vblanks": hist(cols["vblanks"]) if "vblanks" in cols else None,
                "batches_p95": p95(cols["n_batches"]),
                "list_kb_max": max(cols["list_kb"]) if "list_kb" in cols else None,
                "ev_rough": sum(cols["ev_rough"]),
                "placeholder_frames": sum(1 for v in cols["ev_placeholders"] if v > 0),
            }
        except Exception as e:  # a short or damaged capture is a note, not a crash of the test
            r["notes"].append(f"capture unreadable: {e}")
    else:
        r["notes"].append("no dashcam capture saved")
    check(r, "ran to its end (GAME done)", r["done"] and not r["wall_out"])
    check(r, "no panic, no crash.txt, no CPU fault", r["panic"] is None and r["crash"] is None and r["fault"] is None)
    check(r, f"she moved 3+ chunks ({r['moved']} cells)", r["moved"] >= 3 * CHUNK_CELLS)
    check(r, "no paint job hit an allocation failure (paint fail 0)", r["paint_fail"] == 0)
    if case == "calm":
        check(r, f"missed vblanks <= 5% of flips ({missed}/{flips} = {r['missed_pct']:.1f}%)", r["missed_pct"] <= 5.0)
        check(r, f"no flip of 4+ vblanks after tick {SETTLE_TICKS} (longest {r['flip_max']})", r["flip_max"] < 4)
        check(r, f"0 swatches after the first view ({r['swatches']})  [WP2]", r["swatches"] == 0)
        if r["cap"]:
            check(r, f"n_ticks all 2 at tpf:2 {r['cap']['n_ticks']}", set(r["cap"]["n_ticks"]) <= {0, 2})
    if case in ("heavy", "heavy-nopaint", "squeeze"):
        check(r, f"list_most <= 192 KB ({r['list_most'] // 1024} KB)  [WP3]", r["list_most"] <= 192 * 1024)
        if r["cap"]:
            check(r, f"batches p95 <= 400 ({r['cap']['batches_p95']})  [WP3]", r["cap"]["batches_p95"] <= 400)
    if case == "heavy":
        check(r, f"<= 2 swatches after the first view ({r['swatches']})  [WP2]", r["swatches"] <= 2)
    if case in RUN_LEGS:
        check(r, f"flat swatches on screen in < 1% of frames ({r['mush_frames']}/{flips} = {r['mush_pct']:.2f}%)",
              r["mush_pct"] < 1.0)
        check(r, f"never 2 s of flat swatches in a row (longest {r['mush_streak']} frames)", r["mush_streak"] <= 60)
        check(r, f"free RAM >= 1.5 MB ({r['free_min'] // 1024} KB)", r["free_min"] >= 1536 * 1024)
    if case == "squeeze":
        check(r, f"patch buffers flat after tick 300 ({r['patches_first']} -> {r['patches_last']})  [WP3]",
              r["patches_flat"])
        check(r, f"free within +-64 KB after tick 300 ({r['free_min'] // 1024}-{r['free_max'] // 1024} KB)",
              r["free_max"] - r["free_min"] <= 128 * 1024)
        check(r, f"the list guard never fired ({r['list_full']})  [WP3]", r["list_full"] == 0)
    if case == "listcap":
        kb = next((int(w.split(":")[1]) for w in CASES[case][3] if w.startswith("listcap:")), 0)
        check(r, f"the list guard fired ({r['list_full']})  [WP3]", r["list_full"] > 0)
        check(r, f"the list never past its cap ({r['list_most']} <= {kb * 1024} B)  [WP3]", r["list_most"] <= kb * 1024)
    return r


def check(r, what, ok):
    r["checks"].append((what, bool(ok)))


def run_case(case, folder, exe, wall_timeout, emu_timeout, capture, legs_override=None, extra=()):
    save = folder / SAVE
    save.mkdir(parents=True, exist_ok=True)
    ticks = CASES[case][2]
    # Heavy: north up the road into Castle.
    attempts = [[(270, 60, ticks)]]
    if case in RUN_LEGS:
        attempts = [RUN_LEGS[case]]
    if case == "calm":
        # The station road runs north-west from 968,1528 to 760,1320 (Julie's), straight and diagonal:
        # one leg along it keeps her walking for the 1800 ticks (about 190 cells, a dozen chunks). If the
        # county changes and that is blocked, west for a third, then north.
        attempts = [
            [(225, 60, ticks)],
            [(180, 60, ticks // 3), (270, ticks // 3, ticks)],
        ]
    if legs_override:
        attempts = [legs_override]
    r = None
    for legs in attempts:
        for f in list(save.glob("capture-*.bin")) + [save / "crash.txt", save / "perf.txt"]:
            f.unlink(missing_ok=True)
        text = script_text(case, legs, capture, extra)
        (folder / "script.txt").write_text(text + "\n")
        (folder / f"script-{case}.txt").write_text(text + "\n")
        print(f"[{case}] running: {text}", flush=True)
        out, wall_out, secs = run_ppsspp(exe, folder, case, wall_timeout, emu_timeout)
        r = analyse(case, folder, out, wall_out, secs, legs)
        if r["moved"] >= 3 * CHUNK_CELLS or r["crash"] or r["panic"] or r["wall_out"]:
            break
        print(f"[{case}] blocked after {r['moved']} cells; trying another leg", flush=True)
    # Keep this case's files beside the logs.
    for name in ("perf.txt", "crash.txt"):
        if (save / name).exists():
            shutil.copy(save / name, folder / f"{case}-{name}")
    for i, c in enumerate(sorted(save.glob("capture-*.bin"))):
        shutil.copy(c, folder / f"{case}-capture-{i}.bin")
    return r


def report(results):
    print()
    for r in results:
        ok = all(c[1] for c in r["checks"])
        print(f"== {r['case']}: {'PASS' if ok else 'FAIL'}  ({r['secs']:.0f} s, legs {r['legs']})")
        for what, good in r["checks"]:
            print(f"   {'ok  ' if good else 'FAIL'} {what}")
        cap = r["cap"]
        print(f"   flips {r['flips']} missed {r['missed']} ({r['missed_pct']:.1f}%) longest flip {r['flip_max']}"
              f" | swatches {r['swatches']} (all {r['swatches_all']}) wait_frames {r['wait_frames']}"
              f" wait_max {r['wait_max']} extra_waits {r['extra_waits']} boosts {r['boosts']}")
        print(f"   list_most {r['list_most'] // 1024} KB list_full {r['list_full']} left out (quads, polys, patches)"
              f" {r['left_out']} paint_fail {r['paint_fail']}"
              f" job {r['job_wall_us'] / 1000:.0f} ms patches {r['patches_first']} -> {r['patches_last']}"
              f" free {r['free_min'] // 1024}-{r['free_max'] // 1024} KB largest>= {r['maxfree_min'] // 1024} KB")
        if r.get("baked"):
            b = r["baked"]
            print(f"   baked: landed {b['landed']} refused {b['refused']} fails {b['fails']} reads {b['reads']}"
                  f" ({b['read_us']} us) decodes {b['decodes']} ({b['decode_us']} us), stand-ins on screen"
                  f" {b['stand_ins_shown']} ({r['stand_ins_late']} after the settle), loader RAM {b['ram'] // 1024} KB")
        if r["cells_per_s"]:
            print(f"   ground: her pace {r['cells_per_s']:.1f} cells/s, {r['chunks_per_s']:.2f} chunks/s come on screen,"
                  f" the painter lands {r['landed_per_s']:.2f}/s ({r['job_wall_us'] / 1000:.0f} ms a job);"
                  f" swatches showing {r['wait_pct']:.2f}% of frames, longest {r['wait_streak']} frames;"
                  f" flat ones {r['mush_pct']:.2f}%, longest {r['mush_streak']} frames")
        if cap:
            print(f"   dashcam: {cap['frames']} frames, pacing {cap['pacing']}, n_ticks {cap['n_ticks']},"
                  f" vblanks {cap['vblanks']}, batches p95 {cap['batches_p95']}, list_kb max {cap['list_kb_max']},"
                  f" ev_rough {cap['ev_rough']}, placeholder frames {cap['placeholder_frames']}")
        for n in r["notes"]:
            print(f"   note: {n}")
        if r["panic"]:
            print(f"   {r['panic']}")
        if r["fault"]:
            print("   CPU fault (PPSSPP's own report; the game's panic hook never ran):")
            print("      " + r["fault"].replace("\n", "\n      "))
            print(f"   last window: tick {r['last_tick']}, {r['last_pace']}")
        if r["crash"]:
            print("   crash.txt:")
            print("      " + r["crash"].rstrip().replace("\n", "\n      "))
    print()
    print(f"{'case':<14}{'result':<8}{'missed%':>8}{'flipmax':>8}{'swatch':>8}{'wait':>6}{'boost':>6}{'list_KB':>8}"
          f"{'full':>6}{'pfail':>6}{'moved':>7}")
    for r in results:
        ok = all(c[1] for c in r["checks"])
        print(f"{r['case']:<14}{'PASS' if ok else 'FAIL':<8}{r['missed_pct']:>8.1f}{r['flip_max']:>8}{r['swatches']:>8}"
              f"{r['wait_frames']:>6}{r['boosts']:>6}{r['list_most'] // 1024:>8}{r['list_full']:>6}{r['paint_fail']:>6}"
              f"{r['moved']:>7}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("dir", type=Path, help="holds psp-game.prx, present.jpt, jane-psp.jpk, jane-psp.jau, county.bp")
    ap.add_argument("--case", default="all", choices=[*CASES, "all"])
    ap.add_argument("--ppsspp", default=PPSSPP, help="PPSSPPHeadless.exe")
    ap.add_argument("--timeout", type=int, default=900, help="emulated seconds (PPSSPPHeadless --timeout)")
    ap.add_argument("--wall-timeout", type=int, default=3600, help="wall seconds a case may take")
    ap.add_argument("--no-capture", action="store_true", help="do not press the dashcam")
    ap.add_argument("--legs", help="the walk's legs in place of the case's: DEG@FROM-TO,...")
    ap.add_argument("--slowpaint", type=int, help="slowpaint:PCT for every case (the runs have 330 of their own)")
    ap.add_argument("--word", action="append", default=[], help="a script word more for every case (noperf: the overlay off)")
    a = ap.parse_args()
    folder = a.dir.resolve()
    for need in ("psp-game.prx", "present.jpt", "jane-psp.jpk", "county.bp"):
        if not (folder / need).exists():
            sys.exit(f"psp-walk-test: {folder / need} is missing")
    cases = list(CASES) if a.case == "all" else [a.case]
    legs = None
    if a.legs:
        legs = [(int(d), int(f), int(t)) for d, f, t in re.findall(r"(-?\d+)@(\d+)-(\d+)", a.legs)]
    extra = ([f"slowpaint:{a.slowpaint}"] if a.slowpaint else []) + a.word
    results = [run_case(c, folder, a.ppsspp, a.wall_timeout, a.timeout, not a.no_capture, legs, extra) for c in cases]
    report(results)
    sys.exit(0 if all(all(c[1] for c in r["checks"]) for r in results) else 1)


if __name__ == "__main__":
    main()
