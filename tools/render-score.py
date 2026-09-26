#!/usr/bin/env python3
"""Render Fracture's original military score using FluidSynth + GeneralUser GS.
Authoring only: python3 tools/render-score.py --soundfont /path/GeneralUser-GS.sf2
Requires NumPy, libfluidsynth and ffmpeg. The game only needs the rendered Oggs.
"""
import argparse
import ctypes as C
import ctypes.util
import hashlib
import json
from pathlib import Path
import tempfile
import subprocess
import wave
import numpy as np

RATE=44100
OUT=Path(__file__).resolve().parent.parent/'crates/sandbox/assets/audio'
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--soundfont',required=True,type=Path)
args=parser.parse_args()
lib=C.CDLL(ctypes.util.find_library('fluidsynth'))
def api(name,result,*params):
    fn=getattr(lib,name);fn.restype=result;fn.argtypes=params;return fn
ptr=C.c_void_p; integer=C.c_int
new_settings=api('new_fluid_settings',ptr)
setnum=api('fluid_settings_setnum',integer,ptr,C.c_char_p,C.c_double)
setint=api('fluid_settings_setint',integer,ptr,C.c_char_p,integer)
new_synth=api('new_fluid_synth',ptr,ptr)
sfload=api('fluid_synth_sfload',integer,ptr,C.c_char_p,integer)
program=api('fluid_synth_program_select',integer,ptr,integer,integer,integer,integer)
cc=api('fluid_synth_cc',integer,ptr,integer,integer,integer)
on=api('fluid_synth_noteon',integer,ptr,integer,integer,integer)
off=api('fluid_synth_noteoff',integer,ptr,integer,integer)
render=api('fluid_synth_write_float',integer,ptr,integer,ptr,integer,integer,ptr,integer,integer)
delete_synth=api('delete_fluid_synth',None,ptr)
delete_settings=api('delete_fluid_settings',None,ptr)


def compose(battle):
    """16-bar D minor military theme; B section opens into F major."""
    rng=np.random.default_rng(1944)
    events=[]
    def note(ch,pitch,beat,duration,velocity):
        velocity=int(np.clip(velocity+rng.integers(-4,5),1,115))
        start=max(0,beat+float(rng.uniform(-.007,.007)))
        events.append((start,1,ch,pitch,velocity))
        events.append((start+duration,0,ch,pitch,0))
    # Horn, low strings, bowed bass, trumpet, trombone, timpani.
    chords=[(50,53,57),(50,53,57),(46,50,53),(45,49,52),
            (50,53,57),(43,46,50),(46,50,53),(45,49,52),
            (53,57,60),(48,52,55),(46,50,53),(48,52,55),
            (50,53,57),(43,46,50),(45,49,52),(45,49,52)]
    phrases=[[(62,2.5),(65,.5),(69,1)],[(67,2),(65,1),(62,1)],
             [(65,3),(62,1)],[(61,2),(57,2)],
             [(62,1.5),(64,.5),(65,2)],[(67,2),(70,2)],
             [(69,3),(65,1)],[(64,2),(61,2)],
             [(65,3),(69,1)],[(67,2),(64,2)],
             [(65,1.5),(62,.5),(65,2)],[(67,3),(64,1)],
             [(69,2),(65,2)],[(67,2),(62,2)],
             [(64,1.5),(61,.5),(57,2)],[(61,3),(57,1)]]
    for bar,chord in enumerate(chords):
        base=bar*4
        for pitch in chord:
            note(1,pitch+12,base,3.85,49 if not battle else 60)
        for beat in [0,2]:
            note(2,chord[0]-12,base+beat,1.7,65 if not battle else 78)
        cursor=base
        for pitch,duration in phrases[bar]:
            note(0,pitch,cursor,duration*.96,72 if not battle else 90)
            if battle or bar>=8:
                note(4,pitch-12,cursor,duration*.91,51 if not battle else 72)
            cursor+=duration
        if bar in [3,7,11,15]:
            note(3,chord[2]+12,base+2,1.75,59 if not battle else 83)
        if battle:
            for step in range(8):
                # Repeated low-string ostinato supports the line, without a high arcade arpeggio.
                note(6,chord[0]+(7 if step%4==2 else 0),base+step*.5,.37,65 if step%2==0 else 49)
            for beat in [0,2,3.5]: note(5,chord[0]-12,base+beat,.7,70)
        elif bar%4==0: note(5,chord[0]-12,base,1.2,48)
        for beat in [0,2]:note(9,36,base+beat,.14,66 if battle else 49)
        for beat in [1,3]:
            note(9,38,base+beat,.1,83 if battle else 60)
            # A quiet grace stroke ahead of each field-snare accent.
            note(9,38,base+beat-.10,.04,31 if battle else 22)
        if battle or bar%4==3:
            for step in range(6):note(9,38,base+3.25+step*.125,.045,32+step*4)
        if bar%8==0:note(9,49,base,.5,60 if battle else 34)
    return events


def music(name,bpm,battle):
    settings=new_settings()
    setnum(settings,b'synth.sample-rate',RATE);setnum(settings,b'synth.gain',.5)
    setnum(settings,b'synth.reverb.room-size',.72);setnum(settings,b'synth.reverb.damp',.65)
    setnum(settings,b'synth.reverb.width',70);setnum(settings,b'synth.reverb.level',.23)
    setint(settings,b'synth.chorus.active',0);setint(settings,b'synth.polyphony',128)
    synth=new_synth(settings)
    sfid=sfload(synth,str(args.soundfont).encode(),1)
    if sfid<0:raise RuntimeError('Cannot load SoundFont')
    for ch,preset,pan,volume in [(0,60,53,102),(1,48,83,83),(2,43,50,96),(3,56,77,78),(4,57,37,86),(5,47,43,85),(6,42,33,83),(9,0,64,86)]:
        if program(synth,ch,sfid,128 if ch==9 else 0,preset)<0:raise RuntimeError(f'Missing preset {preset}')
        cc(synth,ch,10,pan);cc(synth,ch,7,volume);cc(synth,ch,91,36 if ch!=9 else 20)
    length=round(64*60/bpm*RATE)
    # Warm up one complete loop, then keep the next. Release/reverb tails wrap naturally.
    events=[]
    for cycle in range(3):
        for beat,kind,ch,pitch,velocity in compose(battle):
            events.append((round(beat*60/bpm*RATE)+cycle*length,kind,ch,pitch,velocity))
    events.sort()
    samples=np.zeros((length*3,2),dtype=np.float32)
    cursor=0
    def run(end):
        nonlocal cursor
        while cursor<end:
            count=min(end-cursor,8192)
            buf=np.zeros((count,2),dtype=np.float32)
            render(synth,count,buf.ctypes.data,0,2,buf.ctypes.data,1,2)
            samples[cursor:cursor+count]=buf;cursor+=count
    for sample,kind,ch,pitch,velocity in events:
        if sample>=len(samples):break
        run(sample)
        if kind:on(synth,ch,pitch,velocity)
        else:off(synth,ch,pitch)
    run(len(samples));delete_synth(synth);delete_settings(settings)
    audio=samples[length:2*length]
    audio-=audio.mean(axis=0)
    audio*=.77/max(float(np.max(np.abs(audio))),.001)
    with tempfile.NamedTemporaryFile(suffix='.wav') as tmp:
        with wave.open(tmp.name,'wb') as wav:
            wav.setnchannels(2);wav.setsampwidth(2);wav.setframerate(RATE)
            wav.writeframes((audio*32767).astype('<i2').tobytes())
        subprocess.run(['ffmpeg','-y','-v','error','-i',tmp.name,'-af','highpass=f=38,lowpass=f=8500','-c:a','libvorbis','-q:a','6',str(OUT/(name+'.ogg'))],check=True)
    return {'seconds':round(length/RATE,4),'loop':True,'bpm':bpm,'key':'D minor / F major','arrangement':'sampled orchestral military score','peak':round(float(np.max(np.abs(audio))),4),'rms':round(float(np.sqrt(np.mean(audio**2))),4)}

manifest=json.loads((OUT/'manifest.json').read_text())
manifest['march']=music('march',88,False)
manifest['battle']=music('battle',132,True)
(OUT/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('Rendered military march (88 BPM) and battle (132 BPM). SoundFont SHA256:',hashlib.sha256(args.soundfont.read_bytes()).hexdigest())
