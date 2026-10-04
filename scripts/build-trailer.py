#!/usr/bin/env python3
"""Edit actual native review captures into the repository's gameplay trailer.
Requires ffmpeg/ffprobe on PATH and Pillow. See docs/TRAILER.md for capture steps.
"""
from pathlib import Path
import json
import shutil
import subprocess
from PIL import Image, ImageDraw, ImageFont, ImageOps

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/media'
WORK = ROOT / 'captures/trailer-edit'
OUT.mkdir(parents=True, exist_ok=True)
WORK.mkdir(parents=True, exist_ok=True)
FFMPEG = shutil.which('ffmpeg')
FFPROBE = shutil.which('ffprobe')
assert FFMPEG and FFPROBE, 'Install FFmpeg before editing the trailer'
FONT = ROOT / 'assets/fonts/GravewakeGothic-Regular.ttf'

def run(args):
    subprocess.run([FFMPEG, '-y', '-v', 'error', *map(str, args)], check=True)

def font(size):
    return ImageFont.truetype(str(FONT), size)

def still(source, name):
    image = Image.open(ROOT / source).convert('RGB')
    image.save(OUT / name, quality=90, optimize=True)

stills = {
    'combat.jpg': 'captures/survival/frame-0100.png',
    'packs.jpg': 'captures/legibility/normal/08-pack-revealed.png',
    'shotgun.jpg': 'captures/legibility/normal/15-armory-01-double.png',
    'melee.jpg': 'captures/legibility/normal/41-armory-27-cleaver.png',
    'occult.jpg': 'captures/legibility/normal/34-armory-20-emberstaff.png',
    'bestiary.jpg': 'captures/survival/species-04.png',
    'powers.jpg': 'captures/legibility/normal/65-powers-2.png',
    'binding.jpg': 'captures/legibility/normal/09-binding-available-tooltip.png',
}
for name, source in stills.items():
    still(source, name)
# The cover is a crop of the real title screen; the trailer retains complete frames.
hero = Image.open(ROOT / 'captures/title-cleanup/title-normal.png').convert('RGB')
hero.crop((0, 78, 1440, 538)).save(OUT / 'hero.jpg', quality=93, optimize=True)
poster = ImageOps.contain(hero, (1280, 800))
d = ImageDraw.Draw(poster)
d.rounded_rectangle((700, 670, 1215, 755), radius=8, fill=(15, 21, 22), outline=(176, 133, 72), width=2)
d.polygon([(732, 693), (732, 732), (766, 712)], fill=(246, 232, 199))
d.text((789, 688), 'Watch the gameplay trailer', font=font(31), fill=(246, 232, 199))
poster.save(OUT / 'trailer-poster.jpg', quality=93, optimize=True)

def card(name, headline, subline, emblem=False):
    im = Image.new('RGB', (1280, 720), (9, 18, 21))
    d = ImageDraw.Draw(im)
    if emblem:
        mark = Image.open(ROOT / 'assets/gravewake-emblem.png').convert('RGBA')
        mark.thumbnail((205, 205))
        im.paste(mark, ((1280-mark.width)//2, 80), mark)
    y = 327 if emblem else 275
    d.text((640, y), headline, font=font(82), fill=(248, 232, 196), anchor='mm')
    d.line((340, y+74, 940, y+74), fill=(155, 116, 65), width=2)
    d.text((640, y+123), subline, font=font(32), fill=(191, 183, 160), anchor='mm')
    if emblem:
        d.text((640, 602), 'github.com/nearbycoder/gravewake', font=font(26), fill=(155, 169, 153), anchor='mm')
    path = WORK / f'{name}.png'
    im.save(path)
    return path

opening = card('opening', 'GRAVEWAKE', 'The Hollow Tithe', True)
arsenal = card('arsenal', '33 weapons. One debt.', 'Iron, fire, venom and stolen souls.')
ending = card('ending', 'Answer the bell.', 'Play the native macOS build.', True)
# Source in/out points are seconds at the original 30 fps. Gameplay is not sped up.
shots = [
    (opening, 0, 2.7, True, 'Identity'),
    (ROOT/'captures/survival-review.mp4', 1.1, 7.0, False, 'Mixed-enemy combat and automatic powers'),
    (ROOT/'captures/motion-review.mp4', 7.0, 4.8, False, 'Shotgun fire and mechanical reload'),
    (ROOT/'captures/anatomy-review.mp4', 0.35, 3.3, False, 'Head hit and physical remains'),
    (ROOT/'captures/anatomy-review.mp4', 16.35, 3.3, False, 'Heavy impact and mesh fracture'),
    (arsenal, 0, 1.5, True, 'Arsenal chapter'),
    (OUT/'melee.jpg', 0, 1.7, True, 'Butcher Cleaver'),
    (OUT/'occult.jpg', 0, 1.7, True, 'Ember Staff'),
    (OUT/'shotgun.jpg', 0, 1.7, True, 'Double shotgun'),
    (ROOT/'captures/motion-review.mp4', 0.0, 6.55, False, 'Pack tear, reveal and equip through actual UI'),
    (ROOT/'captures/survival-review.mp4', 11.7, 4.4, False, 'Soul collection and power selection'),
    (ending, 0, 3.0, True, 'End card'),
]
manifest = []
for index, (source, start, duration, is_still, description) in enumerate(shots):
    output = WORK / f'{index:02}.mp4'
    inputs = ['-loop', '1', '-framerate', '30', '-i', source, '-f', 'lavfi', '-i', 'anullsrc=r=48000:cl=stereo'] if is_still else ['-ss', start, '-i', source]
    video = f'scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:color=0x091215,setsar=1,fps=30,fade=t=in:d=0.12,fade=t=out:st={duration-0.12}:d=0.12'
    audio = f'aresample=48000,afade=t=in:d=0.06,afade=t=out:st={duration-0.1}:d=0.1'
    run([*inputs, '-t', duration, '-map', '0:v:0', '-map', '1:a:0' if is_still else '0:a:0', '-vf', video, '-af', audio, '-c:v', 'libx264', '-preset', 'fast', '-crf', '21', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-b:a', '128k', '-ar', '48000', '-ac', '2', output])
    manifest.append({'source': str(source.relative_to(ROOT)), 'in_seconds': start, 'duration_seconds': duration, 'description': description, 'still': is_still})
concat = WORK / 'concat.txt'
concat.write_text(''.join(f"file '{index:02}.mp4'\n" for index in range(len(shots))))
run(['-f', 'concat', '-safe', '0', '-i', concat, '-c', 'copy', '-movflags', '+faststart', OUT/'gravewake-trailer.mp4'])
# Small universal attachment copy for the GitHub inline player (under 10 MB).
run(['-i', OUT/'gravewake-trailer.mp4', '-c:v', 'libx264', '-preset', 'slow', '-b:v', '1450k', '-maxrate', '1600k', '-bufsize', '3200k', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-b:a', '96k', '-movflags', '+faststart', WORK/'gravewake-trailer-inline.mp4'])
assert (WORK/'gravewake-trailer-inline.mp4').stat().st_size < 10_000_000
# Two-pass palette generation keeps the README preview compact and readable.
run(['-ss', '3', '-i', OUT/'gravewake-trailer.mp4', '-t', '8', '-vf', 'fps=10,scale=640:-1:flags=lanczos,palettegen=max_colors=160', '-frames:v', '1', WORK/'palette.png'])
run(['-ss', '3', '-i', OUT/'gravewake-trailer.mp4', '-i', WORK/'palette.png', '-t', '8', '-lavfi', 'fps=10,scale=640:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3', '-loop', '0', OUT/'gameplay-preview.gif'])
(OUT/'trailer-manifest.json').write_text(json.dumps({'fps': 30, 'frame_size': [1280,720], 'source': 'Native Gravewake Metal renderer; staged disposable review runs', 'audio': 'In-game audio only; no external music', 'shots': manifest}, indent=2)+'\n')
print('Trailer and gallery built:', OUT)
