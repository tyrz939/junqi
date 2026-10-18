"""THROWAWAY (PLAN-psp-streaming.md): generic codecs on the console-form chunk dump written by
chunkcodec.rs (u16 n, n*u32 clut, 65536 idx, 65536 relative height per chunk).
python -I codecs.py DUMP [max_chunks]"""
import sys, time, zlib, lzma, bz2, struct
import numpy as np
import zstandard as zstd

S = 256
N = S * S


def load(path, most):
    out = []
    with open(path, "rb") as f:
        while len(out) < most:
            h = f.read(2)
            if not h:
                break
            n = struct.unpack("<H", h)[0]
            clut = f.read(4 * n)
            idx = f.read(N)
            hgt = f.read(N)
            out.append((clut, idx, hgt))
    return out


def predict(idx):
    """Neighbour-match transform: 0..3 = which distinct of L,U,UR,UL matches, else 4 + literal
    goes to a side stream. Returns (sym bytes, literal bytes)."""
    p = np.frombuffer(idx, np.uint8).reshape(S, S).astype(np.int16)
    pad = np.full((S + 1, S + 2), -1, np.int16)
    pad[1:, 1:-1] = p
    L = pad[1:, :-2]; U = pad[:-1, 1:-1]; UR = pad[:-1, 2:]; UL = pad[:-1, :-2]
    cand = [L, U, UR, UL]
    sym = np.full((S, S), 4, np.uint8)
    taken = np.zeros((S, S), bool)
    seen = []
    for k, c in enumerate(cand):
        dup = np.zeros((S, S), bool)
        for s in seen:
            dup |= c == s
        hit = (c == p) & ~taken & ~dup & (c >= 0)
        # rank among distinct candidates
        rank = np.zeros((S, S), np.uint8)
        for j, s in enumerate(cand[:k]):
            d2 = np.zeros((S, S), bool)
            for s2 in cand[:j]:
                d2 |= s == s2
            rank += ((~d2) & (s >= 0)).astype(np.uint8)
        sym[hit] = rank[hit]
        taken |= hit
        seen.append(c)
    lit = p[~taken].astype(np.uint8)
    return sym.tobytes(), lit.tobytes()


def timed(fn, reps=3):
    best = 1e9
    for _ in range(reps):
        t = time.perf_counter()
        fn()
        best = min(best, time.perf_counter() - t)
    return best


def main():
    cks = load(sys.argv[1], 2000)
    k = int(sys.argv[2]) if len(sys.argv) > 2 else 6
    train, test = cks[0::k], cks[3::k]
    print(f"{len(test)} test chunks ({len(train)} train)")
    dict_idx = zstd.train_dictionary(112 * 1024, [c[1] for c in train[:200]])
    dict_h = zstd.train_dictionary(64 * 1024, [c[2] for c in train[:200]])
    pre = [predict(c[1]) for c in test]
    dict_sym = zstd.train_dictionary(64 * 1024, [predict(c[1])[0] for c in train[:120]])
    codecs = []
    for lv in (3, 19, 22):
        codecs.append((f"zstd-{lv}", lambda b, lv=lv: zstd.ZstdCompressor(level=lv).compress(b), lambda c: zstd.ZstdDecompressor().decompress(c)))
    codecs.append(("zstd-22+dict", lambda b: zstd.ZstdCompressor(level=22, dict_data=dict_idx).compress(b),
                   lambda c: zstd.ZstdDecompressor(dict_data=dict_idx).decompress(c)))
    codecs.append(("zlib-9", lambda b: zlib.compress(b, 9), zlib.decompress))
    codecs.append(("bz2-9", lambda b: bz2.compress(b, 9), bz2.decompress))
    flt = [{"id": lzma.FILTER_LZMA2, "preset": 9 | lzma.PRESET_EXTREME, "lc": 0, "lp": 0, "pb": 0}]
    codecs.append(("lzma-9e", lambda b: lzma.compress(b, format=lzma.FORMAT_RAW, filters=flt),
                   lambda c: lzma.decompress(c, format=lzma.FORMAT_RAW, filters=flt)))
    for name, enc, dec in codecs:
        tot, dt = 0, 0.0
        for c in test:
            e = enc(c[1])
            tot += len(e)
            dt += timed(lambda: dec(e), 2)
        print(f"idx  {name:14s} {tot / len(test) / 1024:6.2f} KB   host decode {dt / len(test) * 1e6:7.0f} us")
    # transformed stream through zstd
    for lv in (19, 22):
        tot, dt = 0, 0.0
        z = zstd.ZstdCompressor(level=lv)
        for s, l in pre:
            e1, e2 = z.compress(s), z.compress(l)
            tot += len(e1) + len(e2)
            dt += timed(lambda: (zstd.ZstdDecompressor().decompress(e1), zstd.ZstdDecompressor().decompress(e2)), 2)
        print(f"idx  pred+zstd-{lv}   {tot / len(test) / 1024:6.2f} KB   host decode {dt / len(test) * 1e6:7.0f} us (+ un-predict pass)")
    tot = 0
    zc = zstd.ZstdCompressor(level=22, dict_data=dict_sym)
    for s, l in pre:
        tot += len(zc.compress(s)) + len(zstd.ZstdCompressor(level=22).compress(l))
    print(f"idx  pred+zstd-22+dict {tot / len(test) / 1024:5.2f} KB")
    # height
    for name, enc, dec in [("zstd-22", lambda b: zstd.ZstdCompressor(level=22).compress(b), zstd.ZstdDecompressor().decompress),
                           ("zstd-22+dict", lambda b: zstd.ZstdCompressor(level=22, dict_data=dict_h).compress(b),
                            zstd.ZstdDecompressor(dict_data=dict_h).decompress),
                           ("lzma-9e", lambda b: lzma.compress(b, format=lzma.FORMAT_RAW, filters=flt),
                            lambda c: lzma.decompress(c, format=lzma.FORMAT_RAW, filters=flt))]:
        tot, dt = 0, 0.0
        for c in test:
            e = enc(c[2])
            tot += len(e)
            dt += timed(lambda: dec(e), 2)
        print(f"hgt  {name:14s} {tot / len(test) / 1024:6.2f} KB   host decode {dt / len(test) * 1e6:7.0f} us")
    # joint idx+height in one frame
    tot = sum(len(zstd.ZstdCompressor(level=22).compress(c[1] + c[2])) for c in test)
    print(f"joint idx+hgt zstd-22 {tot / len(test) / 1024:6.2f} KB")
    # neighbour as prefix (previous test chunk in the dump's row order)
    tot = 0
    for a, b in zip(test, test[1:]):
        d = zstd.ZstdCompressionDict(a[1], dict_type=zstd.DICT_TYPE_RAWCONTENT)
        tot += len(zstd.ZstdCompressor(level=22, dict_data=d).compress(b[1]))
    print(f"idx  zstd-22 prefixed by a neighbour {tot / (len(test) - 1) / 1024:6.2f} KB")
    clut = sum(len(c[0]) for c in test) / len(test)
    print(f"clut raw {clut:.0f} B, zstd-19 {sum(len(zstd.ZstdCompressor(level=19).compress(c[0])) for c in test) / len(test):.0f} B")


main()
