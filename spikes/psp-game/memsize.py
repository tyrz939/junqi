"""Sets MEMSIZE=1 in an EBOOT.PBP's PARAM.SFO (PORT.md 13.12).

A PSP-2000 or later gives a homebrew EBOOT its larger user memory (about 52 MB, not 24) only
when its PARAM.SFO says MEMSIZE=1; PPSSPP does the same. cargo-psp's mksfo does not take that
key, so this rewrites the SFO inside the PBP. Usage: python memsize.py <EBOOT.PBP>
"""
import struct
import sys


def read_sfo(b):
    magic, ver, key_start, data_start, count = struct.unpack_from("<4sIIII", b, 0)
    assert magic == b"\0PSF", "not a PARAM.SFO"
    entries = []
    for i in range(count):
        koff, fmt, length, maxlen, doff = struct.unpack_from("<HHIII", b, 20 + 16 * i)
        k = b[key_start + koff:b.index(b"\0", key_start + koff)].decode()
        entries.append((k, fmt, b[data_start + doff:data_start + doff + maxlen], length))
    return ver, entries


def write_sfo(ver, entries):
    entries = sorted(entries, key=lambda e: e[0])
    keys = b""
    data = b""
    index = b""
    for k, fmt, value, length in entries:
        index += struct.pack("<HHIII", len(keys), fmt, length, len(value), len(data))
        keys += k.encode() + b"\0"
        data += value
    while len(keys) % 4:
        keys += b"\0"
    key_start = 20 + len(index)
    data_start = key_start + len(keys)
    return struct.pack("<4sIIII", b"\0PSF", ver, key_start, data_start, len(entries)) + index + keys + data


def main(path):
    pbp = bytearray(open(path, "rb").read())
    assert pbp[:4] == b"\0PBP", "not a PBP"
    offs = list(struct.unpack_from("<8I", pbp, 8))
    sfo = bytes(pbp[offs[0]:offs[1]])
    ver, entries = read_sfo(sfo)
    entries = [e for e in entries if e[0] != "MEMSIZE"]
    entries.append(("MEMSIZE", 0x0404, struct.pack("<I", 1), 4))
    new = write_sfo(ver, entries)
    delta = len(new) - len(sfo)
    out = bytes(pbp[:offs[0]]) + new + bytes(pbp[offs[1]:])
    offs = [offs[0]] + [o + delta for o in offs[1:]]
    out = bytearray(out)
    struct.pack_into("<8I", out, 8, *offs)
    open(path, "wb").write(out)
    print(f"{path}: MEMSIZE=1 ({len(entries)} SFO keys)")


if __name__ == "__main__":
    main(sys.argv[1])
