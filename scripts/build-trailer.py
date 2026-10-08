#!/usr/bin/env python3
"""Edit native review captures into the repository's gameplay trailer, its
poster, the README teaser GIF and the README gallery.
Run scripts/capture-trailer.sh first. Requires ffmpeg/ffprobe on PATH and
Pillow. See docs/TRAILER.md.
"""
from pathlib import Path
import json
import re
import shutil
import subprocess
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/media'
CAPTURES = ROOT / 'captures/trailer'
WORK = CAPTURES / 'edit'
STILLS = CAPTURES / 'run-text/captures/legibility/1920x1080'
SURVIVAL = CAPTURES / 'run-survival/captures/survival'
CALM = CAPTURES / 'run-audio/captures/audio/music-calm.wav'
OUT.mkdir(parents=True, exist_ok=True)
WORK.mkdir(parents=True, exist_ok=True)
FFMPEG = shutil.which('ffmpeg')
FFPROBE = shutil.which('ffprobe')
assert FFMPEG and FFPROBE, 'Install FFmpeg before editing the trailer'
FONT = ROOT / 'assets/fonts/GravewakeGothic-Regular.ttf'
W, H, FPS = 1920, 1080, 30
INK = (248, 232, 196)
BRASS = (176, 133, 72)
NIGHT = (9, 18, 21)


def run(args):
    subprocess.run([FFMPEG, '-y', '-v', 'error', *map(str, args)], check=True)


def font(size):
    return ImageFont.truetype(str(FONT), size)


def still(name):
    """A text-review capture by fixture name (files carry an index prefix)."""
    matches = sorted(STILLS.glob(f'*-{name}.png'))
    assert len(matches) == 1, (name, matches)
    return matches[0]


def jpeg(source, name, width=1600, crop=None):
    image = Image.open(source).convert('RGB')
    if crop:
        image = image.crop(crop)
    if image.width > width:
        image = image.resize((width, round(image.height * width / image.width)), Image.LANCZOS)
    image.save(OUT / name, quality=88, optimize=True, progressive=True)


# README gallery: real 1920×1080 frames at the captured fidelity step.
gallery = {
    'combat.jpg': SURVIVAL / 'frame-0082.png',
    'packs.jpg': still('pack-revealed'),
    'shotgun.jpg': still('armory-01-double'),
    'melee.jpg': still('armory-27-cleaver'),
    'occult.jpg': still('armory-20-emberstaff'),
    'bestiary.jpg': still('bestiary-04'),
    'powers.jpg': still('powers-2'),
    'binding.jpg': still('binding-available-tooltip'),
    'collector.jpg': still('collector-next-descent-08'),
    'pause-ledger.jpg': still('pause-ledger-full'),
    'display.jpg': still('display-ultra'),
    'death-recap.jpg': still('death'),
}
for name, source in gallery.items():
    jpeg(source, name)
title = still('title-new')
# The cover is a crop of the real title screen; the trailer keeps whole frames.
jpeg(title, 'hero.jpg', width=1920, crop=(0, 90, 1920, 650))
poster = Image.open(title).convert('RGB')
d = ImageDraw.Draw(poster)
d.rounded_rectangle((1150, 930, 1830, 1030), radius=10, fill=(15, 21, 22), outline=BRASS, width=3)
d.polygon([(1190, 955), (1190, 1005), (1233, 980)], fill=INK)
d.text((1262, 980), 'Watch the gameplay trailer', font=font(40), fill=INK, anchor='lm')
poster.save(OUT / 'trailer-poster.jpg', quality=88, optimize=True, progressive=True)


def card(name, headline, subline, emblem=False, footer=None):
    im = Image.new('RGB', (W, H), NIGHT)
    d = ImageDraw.Draw(im)
    if emblem:
        mark = Image.open(ROOT / 'assets/gravewake-emblem.png').convert('RGBA')
        mark.thumbnail((300, 300))
        im.paste(mark, ((W - mark.width) // 2, 110), mark)
    y = 500 if emblem else 430
    d.text((W // 2, y), headline, font=font(120), fill=INK, anchor='mm')
    d.line((510, y + 108, 1410, y + 108), fill=(155, 116, 65), width=3)
    d.text((W // 2, y + 182), subline, font=font(48), fill=(191, 183, 160), anchor='mm')
    if footer:
        d.text((W // 2, 930), footer, font=font(38), fill=(155, 169, 153), anchor='mm')
    path = WORK / f'{name}.png'
    im.save(path)
    return path


def caption_band(text, y):
    """A caption in the game's font on a dark band centred at height `y`, as
    a transparent overlay. Arena shots set it above the controls reminder and
    the HUD's bottom row; the pack table below its cards."""
    im = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    f = font(46)
    width = d.textlength(text, font=f)
    x0, x1 = (W - width) / 2 - 40, (W + width) / 2 + 40
    d.rounded_rectangle((x0, y - 40, x1, y + 40), radius=8, fill=(9, 14, 16, 205), outline=BRASS + (230,), width=2)
    d.text((W // 2, y), text, font=f, fill=INK, anchor='mm')
    return im


def framed_still(name, text):
    """A whole native screenshot, slightly reduced, with its caption beneath
    so the caption never covers the interface."""
    im = Image.new('RGB', (W, H), NIGHT)
    shot = Image.open(still(name)).convert('RGB').resize((1632, 918), Image.LANCZOS)
    im.paste(shot, ((W - 1632) // 2, 24))
    d = ImageDraw.Draw(im)
    d.rectangle(((W - 1632) // 2 - 2, 22, (W + 1632) // 2 + 1, 943), outline=(70, 56, 36), width=2)
    d.text((W // 2, 1006), text, font=font(50), fill=INK, anchor='mm')
    path = WORK / f'still-{name}.png'
    im.save(path)
    return path


def overlay(text, y):
    path = WORK / f"caption-{re.sub(r'[^a-z0-9]+', '-', text.lower()).strip('-')}.png"
    caption_band(text, y).save(path)
    return path


opening = card('opening', 'GRAVEWAKE', 'The Hollow Tithe', emblem=True)
arsenal = card('arsenal', '33 weapons. One debt.', 'Iron, fire, venom and stolen souls.')
ending = card('ending', 'Answer the bell.', 'Build and play it on Linux or macOS.', emblem=True,
              footer='github.com/nearbycoder/gravewake')
MOTION = CAPTURES / 'motion-review.mp4'
SURV = CAPTURES / 'survival-review.mp4'
ANATOMY = CAPTURES / 'anatomy-review.mp4'
# (source, in-point s, duration s, captions [(from, to, text)], description).
# Sources that are images are stills; in-points are seconds of the 30 fps
# review recordings, which play at their recorded speed.
shots = [
    (opening, 0, 2.2, [], 'Identity'),
    (SURV, 0.5, 3.3, [(0.3, 3.2, 'Hold the line against twelve kinds of dead')],
     'Mixed-enemy combat and automatic powers at Ultra'),
    (SURV, 4.8, 3.1, [(0.3, 3.0, 'Ultra fidelity: living fire and brazier shadows')],
     'The fight resumes past a brazier at Ultra'),
    (MOTION, 7.0, 4.5, [(0.3, 4.4, 'Break the breech. Eject. Reload.')], 'Shotgun fire and mechanical reload'),
    (ANATOMY, 0.35, 3.0, [], 'Head hit and physical remains (the review names each scene in the game\'s notice bar)'),
    (ANATOMY, 16.35, 3.0, [], 'Heavy impact and mesh fracture'),
    (arsenal, 0, 1.4, [], 'Arsenal chapter'),
    (MOTION, 0.0, 6.2, [(0.3, 3.0, 'Tear a pack. Three cards. Keep one.'),
                        (3.2, 6.1, 'Every card shows its reach and damage against yours')],
     'Pack tear, reveal and equip through the real UI, with menu sounds'),
    (framed_still('collector-next-descent-08', "The Collector's table shows what waits below"), 0, 2.0, [],
     'The Collector previewing the next descent'),
    # The level-up screen names itself, and its cards leave no room for a caption.
    (SURV, 12.4, 3.8, [], 'Soul collection and power selection over the blurred arena'),
    (framed_still('hud-creature-note', 'First sightings name each creature and how to fight it'), 0, 1.8, [],
     'First-sighting creature note'),
    (framed_still('pause-ledger-full', 'Pause to read the run so far and every power you hold'), 0, 1.8, [],
     'Pause ledger over the blurred arena'),
    (framed_still('display-ultra', 'Graphics Fidelity: Low, Medium, High or Ultra'), 0, 2.0, [],
     'Journal Display page at Ultra'),
    (framed_still('death', 'Every ending names the blow that killed you'), 0, 1.8, [], 'Death recap'),
    (framed_still('title-chronicle', 'The title keeps your records and recent runs'), 0, 1.8, [],
     'Title with records and the chronicle'),
    (ending, 0, 3.0, [], 'End card'),
]
manifest = []
calm_at = 0.0  # the calm layer continues from one still to the next
for index, (source, start, duration, captions, description) in enumerate(shots):
    output = WORK / f'{index:02}.mp4'
    is_still = source.suffix == '.png'
    fade = f'fade=t=in:d=0.15,fade=t=out:st={duration - 0.15:.3f}:d=0.15'
    if is_still:
        inputs = ['-loop', '1', '-framerate', FPS, '-i', source, '-ss', calm_at, '-i', CALM]
        calm_at += duration
        # The game's own score under its menus, at the level a player hears
        # it with the default volumes (the reviews' mix: 0.8 / 0.4 × 0.3).
        audio = f'[1:a]aresample=48000,volume=0.6,afade=t=in:d=0.08,afade=t=out:st={duration - 0.12:.3f}:d=0.12[a]'
        video = f'[0:v]fps={FPS},format=yuv420p,{fade}[v]'
    else:
        inputs = ['-ss', start, '-t', duration, '-i', source]
        audio = f'[0:a]aresample=48000,afade=t=in:d=0.06,afade=t=out:st={duration - 0.1:.3f}:d=0.1[a]'
        chain, last = [], '0:v'
        # Arena HUDs keep 740–1080 busy; the pack table is clear lower down.
        y = 846 if source == MOTION and start < 7 else 700
        for k, (a, b, text) in enumerate(captions):
            inputs += ['-loop', '1', '-framerate', FPS, '-i', overlay(text, y)]
            chain.append(f"[{last}][{k + 1}:v]overlay=0:0:shortest=1:enable='between(t,{a},{b})'[c{k}]")
            last = f'c{k}'
        chain.append(f'[{last}]scale={W}:{H},setsar=1,fps={FPS},format=yuv420p,{fade}[v]')
        video = ';'.join(chain)
    run([*inputs, '-t', duration, '-filter_complex', f'{video};{audio}', '-map', '[v]', '-map', '[a]',
         '-c:v', 'libx264', '-preset', 'slow', '-crf', '17', '-pix_fmt', 'yuv420p',
         '-c:a', 'pcm_s16le', '-ar', '48000', '-ac', '2', output.with_suffix('.mkv')])
    manifest.append({
        'source': str(source.relative_to(ROOT)), 'in_seconds': start, 'duration_seconds': duration,
        'captions': [text for _, _, text in captions], 'description': description, 'still': is_still,
    })
concat = WORK / 'concat.txt'
concat.write_text(''.join(f"file '{index:02}.mkv'\n" for index in range(len(shots))))
run(['-f', 'concat', '-safe', '0', '-i', concat, '-c', 'copy', WORK / 'joined.mkv'])
# Loudness: measure the joined edit and bring it to -16 LUFS, then a fast
# limiter at -4.4 dBFS holds the shotgun blasts; after AAC encoding the
# result is about -18.5 LUFS with true peaks near -2 dBTP. (Driving the
# limiter harder only raises AAC's overshoot.)
measure = subprocess.run([FFMPEG, '-hide_banner', '-nostats', '-i', WORK / 'joined.mkv', '-af', 'ebur128',
                          '-f', 'null', '-'], capture_output=True, text=True, check=True).stderr
input_i = float(re.findall(r'I:\s+(-?[\d.]+) LUFS', measure)[-1])
loudnorm = f'volume={-16 - input_i:.2f}dB,alimiter=limit=0.6:attack=1:release=60:level=false:asc=1,aresample=48000'
stats = {'input_i': input_i}
# Capped bit rate keeps the 1080p file well under GitHub's 50 MB warning.
run(['-i', WORK / 'joined.mkv', '-c:v', 'libx264', '-preset', 'slow', '-crf', '19', '-maxrate', '7000k',
     '-bufsize', '14000k', '-pix_fmt', 'yuv420p', '-profile:v', 'high', '-af', loudnorm,
     '-c:a', 'aac', '-b:a', '192k', '-ar', '48000', '-movflags', '+faststart', OUT / 'gravewake-trailer.mp4'])
size = (OUT / 'gravewake-trailer.mp4').stat().st_size
assert size <= 45_000_000, f'trailer is {size} bytes'
# Two-pass palette generation keeps the teaser compact and readable.
gif = ['-ss', '2.6', '-t', '8', '-i', OUT / 'gravewake-trailer.mp4']
run([*gif, '-vf', 'fps=10,scale=640:-1:flags=lanczos,palettegen=max_colors=160', '-frames:v', '1', WORK / 'palette.png'])
run([*gif, '-i', WORK / 'palette.png', '-lavfi',
     'fps=10,scale=640:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3', '-loop', '0',
     OUT / 'gameplay-preview.gif'])
assert (OUT / 'gameplay-preview.gif').stat().st_size <= 10_000_000
fidelity = re.search(r'CAPTURE: fidelity (\w+)', (CAPTURES / 'survival.log').read_text()).group(1)
(OUT / 'trailer-manifest.json').write_text(json.dumps({
    'fps': FPS, 'frame_size': [W, H], 'fidelity': fidelity,
    'source': 'Native Gravewake renderer (wgpu, Vulkan on Linux) in a private virtual KWin; staged disposable review runs',
    'audio': "In-game sound effects, menu cues, ambience and the game's adaptive score; no narration or external music",
    'loudness': {'gain_to_lufs': -16, 'limiter_dbfs': -4.4, 'measured_input_lufs': float(stats['input_i'])},
    'shots': manifest,
}, indent=2) + '\n')
print('Trailer, poster, teaser and gallery built:', OUT, f'({size / 1e6:.1f} MB, fidelity {fidelity})')
