"""Contact sheets for the art director's review (ART.md §3.1, `tools/art-review.sh`).

    python tools/ad-contact.py OUT LABEL=DIR [LABEL=DIR ...]

Each DIR is one render of the review set (`tools/art-review.sh DIR`), oldest first. For every
frame name in any of them, OUT/<scene>/ gets a copy of each stage's frame (`01-before.png`,
`02-...`) and OUT/contact/<scene>.png the stages side by side at 1x, each under its label, so a
scene's before, midway and after read in one image. A stage without that frame is left out.
"""

import os
import shutil
import sys

from PIL import Image, ImageDraw, ImageFont


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    out = sys.argv[1]
    stages = [a.split('=', 1) for a in sys.argv[2:]]
    names = sorted({n for _, d in stages if os.path.isdir(d) for n in os.listdir(d) if n.endswith('.png')})
    os.makedirs(os.path.join(out, 'contact'), exist_ok=True)
    try:
        font = ImageFont.load_default(size=22)
    except TypeError:
        font = ImageFont.load_default()
    for name in names:
        scene = name[:-4]
        have = [(label, os.path.join(d, name)) for label, d in stages if os.path.isfile(os.path.join(d, name))]
        os.makedirs(os.path.join(out, scene), exist_ok=True)
        frames = []
        for i, (label, path) in enumerate(have):
            shutil.copyfile(path, os.path.join(out, scene, f'{i + 1:02d}-{label}.png'))
            frames.append((label, Image.open(path).convert('RGB')))
        bar, gap = 34, 8
        w = sum(im.width for _, im in frames) + gap * (len(frames) - 1)
        h = max(im.height for _, im in frames) + bar
        sheet = Image.new('RGB', (w, h), (18, 18, 24))
        draw = ImageDraw.Draw(sheet)
        x = 0
        for label, im in frames:
            draw.text((x + 8, 5), f'{scene}: {label}', fill=(236, 232, 220), font=font)
            sheet.paste(im, (x, bar))
            x += im.width + gap
        sheet.save(os.path.join(out, 'contact', f'{scene}.png'))
        print(os.path.join(out, 'contact', f'{scene}.png'))


if __name__ == '__main__':
    main()
