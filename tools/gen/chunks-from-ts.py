"""Write data/chunks/*.chunk from a dump of jane/src/world/chunks.ts, or check them against it.

    python3 tools/gen/chunks-from-ts.py DUMP.json write   # (re)write every .chunk from the TypeScript
    python3 tools/gen/chunks-from-ts.py DUMP.json check   # parse every .chunk and compare with it

DUMP.json is tools/gen/chunks-from-ts.ts's output. The .chunk files are the source of truth now;
`write` is how they were made (and would undo hand edits), `check` shows what differs from the
TypeScript on purpose: the graveyard's coffins are a fill, its claim mask leaves them out, and the
town's ginger cat starts on its beat instead of on a lamp post. The format is the doc comment of
crates/jane-schema/src/compile/tables/chunks/parse.rs.
"""
import json, glob, sys, unicodedata, os

R = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
OUT = R + '/data/chunks'

defs = {}
for f in [R + '/data/props.json'] + sorted(glob.glob(R + '/data/props/*.json')):
    defs.update(json.load(open(f)))

dump = json.load(open(sys.argv[1])) if len(sys.argv) > 1 else {}

TILE_CH = {
    'Grass': ',', 'Dirt': ':', 'Road': '=', 'Cobble': '+', 'Fence': '|', 'HouseRoof': '^', 'HouseWall': '&',
    'Garden': '*', 'Rail': '!', 'Cliff': '%', 'Wall': '#', 'TempleWall': '$', 'Tree': '@',
}
FACING = {0: 'east', 1: 'south', 2: 'west', 3: 'north'}

META = {
    'station': dict(face='e', pin='w 4', around=['halt_approach'], doc=[
        "Castle Halt: where the Sunday train stops. One train a week. New Game stands her on the platform.",
        "A halt, not a parade ground: a platform along the line with a shelter, a name board, a bench and",
        "two lamps; the ticket office nobody staffs, with the lost-property things in front of it; a yard",
        "with a fire and a sign; a path out to the road. The box hugs the line at x 4 (`pin`); the line",
        "goes on beyond it both ways, to the county's edge. The path is claimed: nothing placed by name",
        "later (the lost-property things) may stand on it. `halt_approach` is wide on purpose: whatever",
        "changes on the platform changes while she is too far away to see it.",
    ]),
    'julie_house': dict(face='w', doc=[
        "Auntie Julie's: the fence, the house, the stoop, the dog, the thing in the far corner. One more",
        "cottage on the station road with a yard for the dog, the hens and the thing in the corner. A",
        "vegetable plot, two apple trees, a hen house, the washing line, the water butt and the wood",
        "against the wall; one lamp by the door; a lane across the yard gate to gate, and one down to the",
        "south. `yard_gate` is just inside the west gate, where the old small county began. The garden",
        "book lies by the barrel, as the quest says, clear of the dog on the step. The quest skeleton",
        "stands in the far corner, in sight of the stoop and out of reach of anyone who only comes in at",
        "the west gate and stands about. Julie's hens keep still.",
    ]),
    'town': dict(face='w', doc=[
        "Castle. Zelda scale: a town you can cross in thirty seconds that is full the whole way. One high",
        "street through a cobbled square; a back lane behind each row of houses; alleys between them.",
        "Twenty-six buildings, three of them with a way in or a reason to knock; a fountain, a memorial,",
        "stalls, gardens, washing, and the people who live here, out of doors until the lamps. The door",
        "facing the fire in the square is Mrs Allen's sister's: its door is a row in data/placements, at",
        "the `allen_door` slot, and the fire stands straight out from it so the note is right about it.",
        "The `washing_*` slots are where Mrs Marsh's washing came down. The ginger cat starts on its",
        "beat in Cross Lane (the TypeScript stood it on the lamp at the square's corner).",
    ]),
    'farm': dict(face='w', doc=[
        "Lowfield Farm: a farmstead. The farmhouse (whose door talks and does not open: a row in",
        "data/placements, at the `farm_door` slot), the barn (which is chained), the hens, a pen of sheep,",
        "the drilled field across the lane, the hay and the cart. The farmer does not come out.",
    ]),
    'car_wood': dict(face='s', doc=["The wood with the car in it. No road comes here."]),
    'gold_mine': dict(face='s', doc=[
        "The Gold Mine: a cliff face, a mouth in it, a sign that is not sure you can leave. The adit is a",
        "second way out, round the east side of the hill, barred from the inside until a row in",
        "data/triggers unbars it. The Company's pay hatch is a stub of wall with nobody behind the gap; the",
        "Company kept a yard: lamps either side of the mouth, a second cart, pit props, stores.",
    ]),
    'graveyard': dict(face='w', fills={'coffins': 'graveyard_coffins'}, doc=[
        "The graveyard: walled, cobbled, tenanted. Stones in rows, the way a parish lays them out, lamps",
        "at both gates, and a few coffins left out (the `graveyard_coffins` fill: where they lie is the",
        "seed's, kept off the path between the gates). The Burial Chamber's stair is not here.",
    ]),
    'burial': dict(face='s', doc=["The stair down: a block of temple wall in open ground, reached by a footpath, never by a road."]),
    'reed_camp': dict(face='w', doc=["A small outpost: a clearing, a fire, something to sit on. The Reedcutters' fire."]),
    'canteen': dict(face='w', doc=["A small outpost under a roof: the works canteen, its fire, its tables and stores."]),
}
LANDMARK_DOC = [
    "A place the story reaches later: its ground, its shape against the sky and a mark, and no door.",
    "The `{id}_door` slot in its face is where data/doors.json sets one once the place behind it",
    "exists; until then the face is blank, because a locked door with nothing behind it would be a lie.",
]
for lm in ['museum', 'library', 'butterfly_forest', 'lake_statue', 'factory', 'school']:
    META[lm] = dict(face='s', doc=[l.replace('{id}', lm) for l in LANDMARK_DOC])

def variants(letter):
    out = [letter]
    for cp in list(range(0xC0, 0x250)):
        ch = chr(cp)
        if unicodedata.normalize('NFD', ch)[0] == letter and ch != letter and unicodedata.category(ch).startswith('L'):
            out.append(ch)
    return out


GREEK = [chr(c) for c in range(0x3B1, 0x3CA)] + [chr(c) for c in range(0x391, 0x3AA) if chr(c) not in 'ΑΒΕΖΗΙΚΜΝΟΡΤΥΧ']
POOL = GREEK + [chr(c) for c in range(0x430, 0x450)]


class Alloc:
    def __init__(self):
        self.used = set(TILE_CH.values()) | {'.', 'x', ' '}

    def get(self, base, upper):
        base = base.removeprefix('town_')
        first = next((c for c in base if c.isalpha()), 'q')
        a, b = (first.upper(), first.lower()) if upper else (first.lower(), first.upper())
        ascii_rest = [ch for ch in 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwyz0123456789']
        for cand in [a, b] + ascii_rest + variants(a)[1:] + variants(b)[1:] + POOL:
            if cand not in self.used:
                self.used.add(cand)
                return cand
        raise SystemExit('out of chars')


def jsonish(v):
    return json.dumps(v, ensure_ascii=False, separators=(', ', ': '))


def claims_of(cid, c):
    """The TS claim mask, less what the graveyard's random coffins claimed (they are a fill now)."""
    if cid != 'graveyard':
        return c['claims']
    W, H = c['box']['w'], c['box']['h']
    m = [['.'] * W for _ in range(H)]
    def claim(x, y, w, h):
        for j in range(y, y + h):
            for i in range(x, x + w):
                if 0 <= i < W and 0 <= j < H:
                    m[j][i] = 'x'
    for p in c['props']:
        if p['def'] != 'coffin':
            claim(p['x'], p['y'], p['fw'], p['fh'])
    for u in c['units']:
        claim(u['x'], u['y'], 1, 1)
    for k in c['marks']:
        claim(k['x'] - 1, k['y'] - 1, 3, 3)
    return [''.join(r) for r in m]


def emit(cid, c):
    meta = META[cid]
    W, H = c['box']['w'], c['box']['h']
    names = c['tileNames']
    al = Alloc()
    legend = []
    tiles = [[TILE_CH[names[str(t)]] for t in row] for row in c['tiles']]
    tile_used = []
    for row in c['tiles']:
        for t in row:
            n = names[str(t)]
            if n not in tile_used:
                tile_used.append(n)
    for n in tile_used:
        legend.append((TILE_CH[n], f'tile {n}'))

    things = [['.'] * W for _ in range(H)]
    namesl = [['.'] * W for _ in range(H)]

    props = list(c['props'])
    if cid == 'graveyard':
        props = [p for p in props if p['def'] != 'coffin']
    anon_char = {}
    for p in props:
        d = defs[p['def']]
        w, h = d.get('w', 1), d.get('h', 1)
        extra = {k: p[k] for k in ('locked', 'keyTag', 'hidden', 'on', 'to', 'loot', 'use', 'release', 'needs', 'talk', 'label', 'nightLock') if k in p}
        if p['anon']:
            sig = (p['def'], json.dumps(extra, sort_keys=True))
            ch = anon_char.get(sig)
            if ch is None:
                ch = anon_char[sig] = al.get(p['def'], True)
                legend.append((ch, f"prop {p['def']}" + (' - ' + jsonish(extra) if extra else '')))
        else:
            ch = al.get(p['key'] if not p['anon'] else p['def'], True)
            key = '-' if p['anon'] else p['key']
            line = f"prop {p['def']} {key}"
            if extra:
                line += ' ' + jsonish(extra)
            legend.append((ch, line))
        for j in range(h):
            for i in range(w):
                assert things[p['y'] + j][p['x'] + i] == '.', (cid, p)
                things[p['y'] + j][p['x'] + i] = ch
    anon_unit = {}
    for u in c['units']:
        x, y = u['x'], u['y']
        if cid == 'town' and u['key'] == 'town_cat':
            x, y = 34, 48
        facing = FACING.get(u.get('facing')) if u.get('facing') is not None else None
        route = u.get('route')
        sig = (u['def'], facing, json.dumps(route))
        if u['anon'] and not route:
            ch = anon_unit.get(sig)
            if ch is None:
                ch = anon_unit[sig] = al.get(u['def'], False)
                line = f"unit {u['def']} -" + (f' {facing}' if facing else '')
                legend.append((ch, line))
        else:
            ch = al.get(u['def'] if u['anon'] else u['key'], False)
            line = f"unit {u['def']} {'-' if u['anon'] else u['key']}"
            if facing:
                line += f' {facing}'
            if route:
                line += ' ' + jsonish(route)
            legend.append((ch, line))
        assert things[y][x] == '.', (cid, u, things[y][x])
        things[y][x] = ch
    for m in c['marks']:
        ch = al.get(m['name'], False)
        f = FACING.get(m.get('facing')) if m.get('facing') is not None else None
        legend.append((ch, f"mark {m['name']}" + (f' {f}' if f else '')))
        assert namesl[m['y']][m['x']] == '.', (cid, m)
        namesl[m['y']][m['x']] = ch
    for r in c['rects']:
        if r['name'] in meta.get('around', []):
            continue
        ch = al.get(r['name'], True)
        legend.append((ch, f"rect {r['name']}"))
        for (x, y) in [(r['x'], r['y']), (r['x'] + r['w'] - 1, r['y'] + r['h'] - 1)]:
            assert namesl[y][x] == '.', (cid, r, namesl[y][x])
            namesl[y][x] = ch
    if cid == 'graveyard':
        ch = al.get('coffins', False)
        legend.append((ch, 'fill graveyard_coffins'))
        my = H // 2
        for y in list(range(3, my - 3)) + list(range(my + 4, H - 3)):
            for x in range(4, W - 4):
                assert namesl[y][x] == '.'
                namesl[y][x] = ch

    head = []
    for l in meta['doc']:
        head.append(f'// {l}')
    head.append(f'id      {cid}')
    head.append(f'box     {W}x{H}')
    head.append('anchor  centre')
    if 'pin' in meta:
        head.append(f"pin     {meta['pin']}")
    head.append(f"face    {meta['face']}")
    gates = []
    for gx, gy in c['gates']:
        if gx == -1:
            gates.append(f'w{gy}')
        elif gx == W:
            gates.append(f'e{gy}')
        elif gy == -1:
            gates.append(f'n{gx}')
        elif gy == H:
            gates.append(f's{gx}')
        else:
            raise SystemExit(f'{cid}: gate {gx},{gy} not on the ring')
    head.append('gates   ' + ', '.join(gates))
    if c['slots']:
        head.append('slots   ' + ', '.join(f"{s['name']} {s['x']} {s['y']}" for s in c['slots']))
    for r in c['rects']:
        if r['name'] in meta.get('around', []):
            n, w_ = -r['y'], -r['x']
            e = r['x'] + r['w'] - W
            s = r['y'] + r['h'] - H
            head.append(f"around  {r['name']} n{n} e{e} s{s} w{w_}")
    body = head + ['grid'] + [''.join(r) for r in tiles]
    if any(ch != '.' for r in things for ch in r):
        body += ['things'] + [''.join(r) for r in things]
    if any(ch != '.' for r in namesl for ch in r):
        body += ['names'] + [''.join(r) for r in namesl]
    body += ['claims'] + claims_of(cid, c)
    body += ['legend'] + [f'{ch} {line}' for ch, line in legend]
    return '\n'.join(body) + '\n'


FW = {v: k for k, v in FACING.items()}


def parse(text):
    part = 'head'
    head, layers, legend = {}, {}, {}
    for line in text.split('\n'):
        t = line.strip()
        if part == 'head':
            if not t or t.startswith('//'):
                continue
            if t in ('grid',):
                part = 'grid'; layers['grid'] = []; continue
            k, v = t.split(None, 1)
            head[k] = v
        elif part == 'legend':
            if not t or t.startswith('//'):
                continue
            ch = t[0]
            legend[ch] = t[1:].strip()
        else:
            if t in ('things', 'names', 'claims', 'legend'):
                part = t
                if t != 'legend':
                    layers[t] = []
                continue
            if t:
                layers[part].append(t)
    return head, layers, legend


def check(cid, c):
    text = open(f'{R}/data/chunks/{cid}.chunk').read()
    head, layers, legend = parse(text)
    W, H = map(int, head['box'].split('x'))
    assert (W, H) == (c['box']['w'], c['box']['h'])
    names = c['tileNames']
    # tiles
    for y in range(H):
        for x in range(W):
            want = names[str(c['tiles'][y][x])]
            got = legend[layers['grid'][y][x]].split()[1]
            assert want == got, (cid, x, y, want, got)
    # things
    props, units = [], []
    th = layers.get('things', ['.' * W] * H)
    taken = set()
    for y in range(H):
        for x in range(W):
            ch = th[y][x]
            if ch == '.' or (x, y) in taken:
                continue
            e = legend[ch]
            kind, rest = e.split(None, 1)
            if kind == 'prop':
                parts = rest.split(None, 2)
                d = parts[0]
                key = parts[1] if len(parts) > 1 and parts[1] != '-' else None
                extra = json.loads(parts[2]) if len(parts) > 2 else {}
                w, h = defs[d].get('w', 1), defs[d].get('h', 1)
                for j in range(h):
                    for i in range(w):
                        assert th[y + j][x + i] == ch and (x + i, y + j) not in taken, (cid, ch, x, y)
                        taken.add((x + i, y + j))
                props.append(dict(def_=d, key=key, x=x, y=y, **extra))
            elif kind == 'unit':
                parts = rest.split(None, 3)
                d = parts[0]
                key = parts[1] if len(parts) > 1 and parts[1] != '-' else None
                facing = FW[parts[2]] if len(parts) > 2 else None
                route = json.loads(parts[3]) if len(parts) > 3 else None
                taken.add((x, y))
                units.append(dict(def_=d, key=key, x=x, y=y, facing=facing, route=route))
    want_props = []
    for p in c['props']:
        if cid == 'graveyard' and p['def'] == 'coffin':
            continue
        extra = {k: p[k] for k in ('locked', 'keyTag', 'hidden', 'on', 'to', 'loot', 'use', 'release', 'needs', 'talk', 'label', 'nightLock') if k in p}
        want_props.append(dict(def_=p['def'], key=None if p['anon'] else p['key'], x=p['x'], y=p['y'], **extra))
    key = lambda p: (p['y'], p['x'])
    assert sorted(want_props, key=key) == sorted(props, key=key), cid
    want_units = []
    for u in c['units']:
        x, y = (34, 48) if u['key'] == 'town_cat' else (u['x'], u['y'])
        want_units.append(dict(def_=u['def'], key=None if u['anon'] else u['key'], x=x, y=y, facing=u.get('facing'), route=u.get('route')))
    assert sorted(want_units, key=key) == sorted(units, key=key), (cid, sorted(want_units, key=key), sorted(units, key=key))
    # names
    nm = layers.get('names', ['.' * W] * H)
    seen = {}
    for y in range(H):
        for x in range(W):
            ch = nm[y][x]
            if ch != '.':
                seen.setdefault(ch, []).append((x, y))
    marks, rects = {}, {}
    for ch, cells in seen.items():
        kind, rest = legend[ch].split(None, 1)
        if kind == 'mark':
            assert len(cells) == 1
            p = rest.split()
            marks[p[0]] = (cells[0][0], cells[0][1], FW[p[1]] if len(p) > 1 else None)
        elif kind == 'rect':
            (x0, y0), (x1, y1) = cells if len(cells) == 2 else (cells[0], cells[0])
            rects[rest] = (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
    assert marks == {m['name']: (m['x'], m['y'], m.get('facing')) for m in c['marks']}, cid
    want_rects = {r['name']: (r['x'], r['y'], r['w'], r['h']) for r in c['rects'] if r['name'] != 'halt_approach'}
    assert rects == want_rects, (cid, rects, want_rects)
    assert layers['claims'] == claims_of(cid, c), cid
    # every legend char used
    used = set(''.join(''.join(l) for l in layers.values()))
    for ch in legend:
        assert ch in used, (cid, ch)
    print('ok', cid, len(props), 'props', len(units), 'units', len(marks), 'marks', len(rects), 'rects', len(legend), 'legend')


if __name__ == '__main__':
    if len(sys.argv) != 3 or sys.argv[2] not in ('write', 'check'):
        raise SystemExit(__doc__)
    for cid, c in dump.items():
        if sys.argv[2] == 'write':
            os.makedirs(OUT, exist_ok=True)
            open(f'{OUT}/{cid}.chunk', 'w').write(emit(cid, c))
        check(cid, c)
