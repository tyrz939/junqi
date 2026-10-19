"""Compress microchunk's raw T8 plane for spikes/psp-game/assets/: zstd -19 and lz4 HC (block).
python tools/psp-measure/microchunk.py RAW spikes/psp-game/assets"""
import sys, zstandard, lz4.block
raw = open(sys.argv[1], "rb").read()
out = sys.argv[2]
open(out + "/chunk.zst", "wb").write(zstandard.ZstdCompressor(level=19, write_content_size=True).compress(raw))
open(out + "/chunk.lz4", "wb").write(lz4.block.compress(raw, mode="high_compression", compression=12, store_size=False))
h = 0xcbf29ce484222325
for b in raw:
    h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
print("raw", len(raw), "fnv1a64 0x%x" % h)
