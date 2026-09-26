#!/usr/bin/env python3
"""Synthesize Fracture sound effects (NumPy + ffmpeg).
No downloaded samples. Seeded synthesis; runtime only needs the bundled Ogg files.
"""
from pathlib import Path
import json
import argparse
import subprocess
import tempfile
import wave
import numpy as np

RATE = 44100
OUT = Path(__file__).resolve().parent.parent / 'crates/sandbox/assets/audio'
rng = np.random.default_rng(8042)
manifest = json.loads((OUT/'manifest.json').read_text()) if (OUT/'manifest.json').exists() else {}
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--only',nargs='+',help='Regenerate just the named effects')
selected=parser.parse_args().only


def timeline(seconds):
    return np.arange(round(seconds * RATE)) / RATE


def hz(note):
    return 440 * 2 ** ((note - 69) / 12)


def envelope(t, attack=.008, release=.08):
    return np.minimum(t / attack, 1) * np.minimum((len(t) / RATE - t) / release, 1)


def noise(n, width=7):
    return np.convolve(rng.uniform(-1, 1, n), np.ones(width) / np.sqrt(width), mode='same')


def voice(note, seconds, kind='brass'):
    t = timeline(seconds)
    f = hz(note)
    if kind == 'brass':
        phase = 2 * np.pi * f * t + .025 * np.sin(2 * np.pi * 5 * t)
        tone = sum(np.sin(phase * h) * np.exp(-h / 4) / h ** .55 for h in range(1, 12))
        return tone * envelope(t, .035, .1) * (.85 + .15 * np.exp(-t * 7))
    if kind == 'strings':
        tone = sum(np.sin(2 * np.pi * f * ratio * t * h) / h ** 1.6
                   for ratio in [.998, 1.002] for h in range(1, 7)) / 2
        return tone * envelope(t, .13, .2)
    if kind == 'bell':
        return (np.sin(2*np.pi*f*t) + .25*np.sin(2*np.pi*f*2.76*t)) * np.exp(-t*5) * envelope(t)
    return (np.sin(2*np.pi*f*t) + .22*np.sin(2*np.pi*f*2*t)) * envelope(t, .018, .07)


def drum(kind):
    t = timeline(.28 if kind == 'snare' else .45)
    if kind == 'snare':
        return (noise(len(t), 2) * .6 + np.sin(2*np.pi*185*t)*.25) * np.exp(-t*24) * envelope(t, .001, .015)
    if kind == 'hat':
        raw = rng.uniform(-1, 1, len(t))
        return (raw - np.roll(raw, 1)) * np.exp(-t*45) * .13
    return np.sin(2*np.pi*(48*t + 2*(1-np.exp(-t*25)))) * np.exp(-t*12) * envelope(t, .002, .02)


def add(buffer, mono, start, gain, pan=0):
    # Wrap tails across the loop boundary, including the final cadence/reverb.
    idx = (np.arange(len(mono)) + round(start*RATE)) % len(buffer)
    buffer[idx, 0] += mono * gain * np.sqrt((1-pan)/2)
    buffer[idx, 1] += mono * gain * np.sqrt((1+pan)/2)


def save(name, samples, loop=False, bpm=None):
    if selected and name not in selected: return
    if samples.ndim == 1:
        samples = np.column_stack([samples, samples])
    samples = samples - samples.mean(axis=0)
    peak = np.max(np.abs(samples))
    samples = np.tanh(samples / max(peak, .001) * 1.2) * .78
    # Remove the abrasive broadband fizz after saturation (which adds harmonics).
    # Offline zero-phase low-pass: no per-frame DSP cost in the game. The FFT
    # treats loops as periodic, so filtering does not introduce a loop seam.
    cutoff = {'ap': 3000, 'impact': 2800, 'fracture': 2400,
              'tracks': 1600, 'fence-crunch': 1800, 'reload': 2200,
              'tracks-road': 1600, 'tracks-sand': 1100,
              'tracks-grass': 1400, 'tracks-bridge': 1300,
              'shrub-break': 1800, 'crate-break': 2000, 'timber-break': 1600}.get(name)
    if cutoff:
        frequencies = np.fft.rfftfreq(len(samples), 1/RATE)
        rolloff = 1 / np.sqrt(1 + (frequencies / cutoff)**12)
        samples = np.fft.irfft(np.fft.rfft(samples, axis=0) * rolloff[:, None],
                               n=len(samples), axis=0)
        peak = np.max(np.abs(samples))
        if peak > .65:
            samples *= .65 / peak
    if not loop:
        samples *= envelope(timeline(len(samples)/RATE), .003, .035)[:, None]
    with tempfile.NamedTemporaryFile(suffix='.wav') as f:
        with wave.open(f.name, 'wb') as wav:
            wav.setnchannels(2); wav.setsampwidth(2); wav.setframerate(RATE)
            wav.writeframes((samples * 32767).astype('<i2').tobytes())
        subprocess.run(['ffmpeg','-y','-v','error','-i',f.name,'-c:a','libvorbis','-q:a','5',str(OUT / (name+'.ogg'))],check=True)
    manifest[name] = {'seconds': round(len(samples)/RATE,4), 'loop': loop,
                      'peak': round(float(np.max(np.abs(samples))),4),
                      'rms': round(float(np.sqrt(np.mean(samples**2))),4)}
    if bpm: manifest[name]['bpm'] = bpm


OUT.mkdir(parents=True, exist_ok=True)
for name, duration in [('he',.75),('ap',.46),('rocket',1.1),('plasma',.85),('impact',.38),('collision',.45),('fracture',.7),('explosion',1.4),('splash',.8)]:
    t=timeline(duration); n=noise(len(t), 13)
    if name=='he': x=.8*n*np.exp(-t*13)+np.sin(2*np.pi*(48*t+2*(1-np.exp(-t*40))))*np.exp(-t*7)
    elif name=='ap': x=noise(len(t),2)*np.exp(-t*35)+.6*np.sin(2*np.pi*135*t)*np.exp(-t*15)+.15*np.sin(2*np.pi*1600*t)*np.exp(-t*30)
    elif name=='rocket': x=n*(1-np.exp(-t*100))*np.exp(-t*3)+.35*np.sin(2*np.pi*(100*t+40*t*t))*np.exp(-t*5)
    elif name=='plasma': x=np.sin(2*np.pi*(75*t+300*(1-np.exp(-t*8))/8))*np.exp(-t*5)+n*np.exp(-t*9)+.3*np.sin(2*np.pi*1040*t)*np.exp(-t*12)
    elif name=='impact': x=noise(len(t),2)*np.exp(-t*35)+sum(np.sin(2*np.pi*f*t)*np.exp(-t*decay)*g for f,decay,g in [(640,18,.5),(1093,24,.3),(180,14,.4)])
    elif name=='collision': x=n*np.exp(-t*17)+.6*np.sin(2*np.pi*85*t)*np.exp(-t*12)+.2*np.sin(2*np.pi*430*t)*np.exp(-t*20)
    elif name=='fracture':
        x=n*np.exp(-t*7)
        for delay in [.03,.07,.13,.22,.34]:
            tt=np.maximum(t-delay,0);x+=noise(len(t),3)*np.exp(-tt*55)*(t>=delay)*.5
    elif name=='splash': x=n*np.exp(-t*4)*(1-np.exp(-t*70))+.2*np.sin(2*np.pi*(90*t+50*t*t))*np.exp(-t*5)
    else: x=n*np.exp(-t*3.8)+.8*np.sin(2*np.pi*(32*t+1.8*(1-np.exp(-t*32))))*np.exp(-t*4)
    save(name,x)
# Seamless loops use integral periods; engine layers crossfade as throttle rises.
t=timeline(2)
for name, rpm in [('idle',28),('engine',48)]:
    phase=2*np.pi*rpm*t
    x=sum(np.sin(phase*h)*(.65**h) for h in range(1,9))*(.8+.2*np.sin(2*np.pi*7*t))
    save(name,x,True)
x=noise(len(t),3)*(.25+.75*np.maximum(0,np.cos(2*np.pi*18*t))**10)
x+=.25*np.sin(2*np.pi*280*t)*np.maximum(0,np.cos(2*np.pi*18*t))**10
# End the random clatter at a quiet part of the tread cycle, with a smooth
# 20 ms join. Do not duplicate an arbitrary noisy chunk at both endpoints.
n=round(.02*RATE); fade=np.sin(np.linspace(0,np.pi/2,n))**2
x[:n]*=fade; x[-n:]*=fade[::-1]
save('tracks',x,True)
save('laser',sum(np.sin(2*np.pi*f*t)*g for f,g in [(110,.25),(440,.3),(880,.14),(1320,.07)])*(.8+.2*np.sin(2*np.pi*16*t)),True)
t=timeline(1)
save('charge',(np.sin(2*np.pi*(150*t+500*t*t))+.3*np.sin(2*np.pi*(301*t+990*t*t)))*(.15+.85*t)*envelope(t,.03,.015))
# Hollow oil-drum buckling: low resonances and three irregular shell collapses.
t=timeline(.78);x=np.zeros(len(t))
for delay,amount in [(0,1),(.11,.7),(.29,.45)]:
    dt=np.maximum(t-delay,0);gate=(t>=delay)
    ring=sum(np.sin(2*np.pi*f*dt)*amp*np.exp(-dt*decay) for f,amp,decay in [(76,.7,9),(143,.5,14),(267,.2,20)])
    x+=gate*amount*(ring+noise(len(t),10)*np.exp(-dt*24)*.65)
save('barrel-crush',x)
# Fence buckling shares the barrel's low, hollow collapses, with a shorter,
# more damped tail for the lighter frame. No pitched sweep or wire whistle.
t=timeline(.60)
fence=x[:len(t)].copy()*np.exp(-t*2.0)
save('fence-crunch',fence)
# Quiet utilitarian pickup and breech-latch cues, replacing the old bell jingles.
t=timeline(.24)
save('pickup',.3*np.sin(2*np.pi*330*t)*np.exp(-t*25)+noise(len(t),8)*np.exp(-t*60)*.25)
t=timeline(.26);x=noise(len(t),3)*np.exp(-t*60)
dt=np.maximum(t-.105,0)
x+=(t>=.105)*(.6*noise(len(t),4)+.2*np.sin(2*np.pi*210*dt))*np.exp(-dt*40)
save('reload',x)
for name, notes in [('respawn',[62,69,74]),('victory',[74,78,81,86]),('defeat',[62,60,57,50]),('countdown',[74]),('go',[86])]:
    length=.13 if name in ('reload','countdown') else .2
    x=np.zeros((round((len(notes)*length+.35)*RATE),2))
    for i,note in enumerate(notes): add(x,voice(note,.35,'bell'),i*length,.5)
    save(name,x)
# Quiet movement textures, blended with the existing tread/engine layers.
# Added at the end to preserve the seeded synthesis of all existing effects.
t=timeline(2)
for name in ['tracks-road','tracks-sand','tracks-grass','tracks-bridge']:
    if name=='tracks-road':
        tread=np.maximum(0,np.cos(2*np.pi*22*t))**8
        x=noise(len(t),25)*(.15+.85*tread)+.12*np.sin(2*np.pi*110*t)*tread
    elif name=='tracks-sand':
        x=noise(len(t),55)*(.65+.2*np.sin(2*np.pi*4*t)+.15*np.sin(2*np.pi*9*t))
    elif name=='tracks-grass':
        x=noise(len(t),30)*(.25+.5*np.sin(2*np.pi*3*t)**2+.25*np.sin(2*np.pi*7*t)**2)
    else:
        tread=np.maximum(0,np.cos(2*np.pi*8*t))**6
        x=(.55*np.sin(2*np.pi*65*t)+.22*np.sin(2*np.pi*130*t)+noise(len(t),18)*.3)*(.25+.75*tread)
    save(name,x*envelope(t,.025,.025),True)
# Small scenery destruction: leafy rustle and short, dry wood crunches.
# Damped fixed resonances avoid pitched sweeps or ringing notification tones.
t=timeline(.28)
x=noise(len(t),18)*np.exp(-t*16)*(1-np.exp(-t*180))
x+=.10*np.sin(2*np.pi*85*t)*np.exp(-t*30)
save('shrub-break',x)
for name,duration,body in [('crate-break',.38,145),('timber-break',.48,92)]:
    t=timeline(duration);x=np.zeros(len(t))
    for delay,amount in [(0,1),(.045,.5),(.12,.25)]:
        dt=np.maximum(t-delay,0);gate=(t>=delay)
        crunch=noise(len(t),12)*np.exp(-dt*45)
        knock=(np.sin(2*np.pi*body*dt)+.25*np.sin(2*np.pi*body*2.7*dt))*np.exp(-dt*28)
        x+=gate*amount*(crunch+.4*knock)
    save(name,x)
(OUT/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(f'Generated {len(manifest)} original audio assets in {OUT}')
