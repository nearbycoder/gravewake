"""Reproducible trims of the CC0 sources documented in assets/audio/SOURCES.md."""
from pathlib import Path
import subprocess, re
ROOT=Path(__file__).resolve().parent.parent
sources=[
 ('shotgun-a','Prepared SFX Library/Mossberg/N_30P.wav',1.690,1.35,-1.5),
 ('shotgun-b','Prepared SFX Library/Mossberg/N_30P.wav',4.954,1.35,-1.5),
 ('pistol','Prepared SFX Library/1911/A_42P.wav',0.935,0.85,-3),
 ('shell-in','ShotgunSounds/Shell in Chamber.mp3',0.505,0.40,-9),
 ('rack','ShotgunSounds/Rack.mp3',0.605,0.48,-6),
]
for name, source, start, duration, target in sources:
 src=ROOT/'reference/audio'/source
 filters=f'highpass=f=45,afade=t=in:d=0.001,afade=t=out:st={duration-0.12}:d=0.12'
 cmd=['ffmpeg','-hide_banner','-ss',str(start),'-t',str(duration),'-i',str(src),'-af',filters+',volumedetect','-f','null','-']
 result=subprocess.run(cmd,capture_output=True,text=True,check=True)
 peak=float(re.search(r'max_volume: ([-\d.]+)',result.stderr).group(1))
 subprocess.run(['ffmpeg','-y','-v','error','-ss',str(start),'-t',str(duration),'-i',str(src),'-af',filters+f',volume={target-peak}dB','-ar','48000','-ac','2','-c:a','pcm_s16le',str(ROOT/'assets/audio'/f'{name}.wav')],check=True)
 print(name, 'trim',start,duration,'gain',round(target-peak,2))
