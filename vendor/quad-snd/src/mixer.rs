use crate::{AudioContext, PlaySoundParams};

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;

enum AudioMessage {
    AddSound(u32, Vec<f32>),
    Play(u32, u32, bool, f32),
    Stop(u32),
    StopAll(u32),
    SetVolume(u32, f32),
    SetVolumeAll(u32, f32),
    Delete(u32),
}

#[derive(Debug)]
pub struct SoundState {
    sound_id: u32,
    play_id: u32,
    sample: usize,
    data: Arc<[f32]>,
    looped: bool,
    volume: f32,
    target_volume: f32,
    stopping: bool,
}

impl SoundState {
    fn rewind(&mut self) {
        self.sample = 0;
    }
}

pub struct Mixer {
    rx: mpsc::Receiver<AudioMessage>,
    sounds: HashMap<u32, Arc<[f32]>>,
    mixer_state: Vec<SoundState>,
}

pub struct MixerBuilder {
    rx: mpsc::Receiver<AudioMessage>,
}

pub struct MixerControl {
    tx: mpsc::Sender<AudioMessage>,
    sound_id: Cell<u32>,
    play_id: Cell<u32>,
}

pub struct Playback {
    play_id: u32,
}

impl Playback {
    pub fn stop(self, ctx: &AudioContext) {
        ctx.mixer_ctrl.send(AudioMessage::Stop(self.play_id));
    }

    pub fn set_volume(&self, ctx: &AudioContext, volume: f32) {
        ctx.mixer_ctrl
            .send(AudioMessage::SetVolume(self.play_id, volume));
    }
}

impl MixerControl {
    pub fn load(&self, data: &[u8]) -> u32 {
        let sound_id = self.sound_id.get();

        let samples = load_samples_from_file(data).unwrap();

        self.tx
            .send(crate::mixer::AudioMessage::AddSound(sound_id, samples))
            .unwrap_or_else(|_| println!("Audio thread died"));
        self.sound_id.set(sound_id + 1);

        sound_id
    }

    pub fn play(&self, sound_id: u32, params: PlaySoundParams) -> Playback {
        let play_id = self.play_id.get();

        self.send(AudioMessage::Play(
            sound_id,
            play_id,
            params.looped,
            params.volume,
        ));

        self.play_id.set(play_id + 1);

        Playback { play_id }
    }

    pub fn stop(&self, play_id: u32) {
        self.send(AudioMessage::Stop(play_id));
    }

    pub fn stop_all(&self, sound_id: u32) {
        self.send(AudioMessage::StopAll(sound_id));
    }

    pub fn set_volume_all(&self, sound_id: u32, volume: f32) {
        self.send(AudioMessage::SetVolumeAll(sound_id, volume));
    }

    pub fn delete(&self, sound_id: u32) {
        self.send(AudioMessage::Delete(sound_id));
    }

    fn send(&self, message: AudioMessage) {
        self.tx
            .send(message)
            .unwrap_or_else(|_| println!("Audio thread died"))
    }
}

impl MixerBuilder {
    pub fn build(self) -> Mixer {
        Mixer {
            rx: self.rx,
            sounds: HashMap::new(),
            mixer_state: vec![],
        }
    }
}

impl Mixer {
    pub fn new() -> (MixerBuilder, MixerControl) {
        let (tx, rx) = mpsc::channel();

        (
            MixerBuilder { rx },
            MixerControl {
                tx,
                sound_id: Cell::new(0),
                play_id: Cell::new(0),
            },
        )
    }

    pub fn fill_audio_buffer(&mut self, buffer: &mut [f32], _frames: usize) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                AudioMessage::AddSound(id, data) => {
                    self.sounds.insert(id, data.into());
                }
                AudioMessage::Play(sound_id, play_id, looped, volume) => {
                    // Bound collision/debris storms without cutting an existing voice.
                    if !looped
                        && (self.mixer_state.iter().filter(|s| !s.looped).count() >= 32
                            || self
                                .mixer_state
                                .iter()
                                .filter(|s| !s.looped && s.sound_id == sound_id)
                                .count()
                                >= 3)
                    {
                        continue;
                    }
                    if let Some(data) = self.sounds.get(&sound_id) {
                        self.mixer_state.push(SoundState {
                            sound_id,
                            play_id,
                            sample: 0,
                            data: data.clone(),
                            looped,
                            volume: 0.0,
                            target_volume: volume.clamp(0.0, 1.0),
                            stopping: false,
                        });
                    }
                }
                AudioMessage::Stop(play_id) => {
                    if let Some(i) = self.mixer_state.iter().position(|s| s.play_id == play_id) {
                        self.mixer_state[i].target_volume = 0.0;
                        self.mixer_state[i].stopping = true;
                    }
                }
                AudioMessage::StopAll(sound_id) => {
                    for i in (0..self.mixer_state.len()).rev() {
                        if self.mixer_state[i].sound_id == sound_id {
                            self.mixer_state[i].target_volume = 0.0;
                            self.mixer_state[i].stopping = true;
                        }
                    }
                }
                AudioMessage::SetVolume(play_id, volume) => {
                    if let Some(sound) = self.mixer_state.iter_mut().find(|s| s.play_id == play_id)
                    {
                        sound.target_volume = volume.clamp(0.0, 1.0);
                    }
                }
                AudioMessage::SetVolumeAll(sound_id, volume) => {
                    for sound in self
                        .mixer_state
                        .iter_mut()
                        .filter(|s| s.sound_id == sound_id)
                    {
                        sound.target_volume = volume.clamp(0.0, 1.0);
                    }
                }
                AudioMessage::Delete(sound_id) => {
                    for i in (0..self.mixer_state.len()).rev() {
                        if self.mixer_state[i].sound_id == sound_id {
                            self.mixer_state.swap_remove(i);
                        }
                    }
                    self.sounds.remove(&sound_id);
                }
            }
        }

        // zeroize the buffer
        buffer.fill(0.0);

        // Append the wrapped head after the tail. Mixing it at buffer[0]
        // overlapped unrelated samples and left a hole at every loop boundary.
        let mut i = 0;
        while let Some(sound) = self.mixer_state.get_mut(i) {
            if sound.data.is_empty() {
                self.mixer_state.swap_remove(i);
                continue;
            }
            let mut written = 0;
            while written < buffer.len() {
                if sound.sample == sound.data.len() {
                    if !sound.looped {
                        break;
                    }
                    sound.rewind();
                }
                let count = (sound.data.len() - sound.sample).min(buffer.len() - written);
                let target = sound.target_volume;
                for frame in 0..count / 2 {
                    // 10 ms at the mixer's fixed 44.1 kHz; avoid zipper noise
                    // and clicks when stopping/muting low-frequency vehicle loops.
                    sound.volume += (target - sound.volume) * (1.0 / 441.0);
                    for channel in 0..2 {
                        buffer[written + frame * 2 + channel] +=
                            sound.data[sound.sample + frame * 2 + channel] * sound.volume;
                    }
                }
                sound.sample += count;
                written += count;
            }
            if (!sound.looped && sound.sample == sound.data.len())
                || (sound.stopping && sound.volume < 0.00001)
            {
                self.mixer_state.swap_remove(i);
            } else {
                i += 1;
            }
        }
        // Leave normal signals untouched; round off exceptional summed peaks.
        for sample in buffer.iter_mut() {
            let amplitude = sample.abs();
            if amplitude > 0.8 {
                *sample = sample.signum() * (0.8 + 0.2 * ((amplitude - 0.8) / 0.2).tanh());
            }
        }
    }
}

/// Parse ogg/wav/etc and get  resampled to 44100, 2 channel data
pub fn load_samples_from_file(bytes: &[u8]) -> Result<Vec<f32>, ()> {
    let mut audio_stream = {
        let file = std::io::Cursor::new(bytes);
        audrey::Reader::new(file).unwrap()
    };

    let description = audio_stream.description();
    let channels_count = description.channel_count();
    assert!(channels_count == 1 || channels_count == 2);

    let mut frames: Vec<f32> = Vec::with_capacity(4096);
    let mut samples_iterator = audio_stream
        .samples::<f32>()
        .map(std::result::Result::unwrap);

    // audrey's frame docs: "TODO: Should consider changing this behaviour to check the audio file's actual number of channels and automatically convert to F's number of channels while reading".
    // lets fix this TODO here
    if channels_count == 1 {
        frames.extend(samples_iterator.flat_map(|sample| [sample, sample]));
    } else if channels_count == 2 {
        frames.extend(samples_iterator);
    }

    let sample_rate = description.sample_rate();

    // stupid nearest-neighbor resampler
    if sample_rate != 44100 {
        let mut new_length = ((44100 as f32 / sample_rate as f32) * frames.len() as f32) as usize;

        // `new_length` must be an even number
        new_length -= new_length % 2;

        let mut resampled = vec![0.0; new_length];

        for (n, sample) in resampled.chunks_exact_mut(2).enumerate() {
            let ix = 2 * ((n as f32 / new_length as f32) * frames.len() as f32) as usize;
            sample[0] = frames[ix];
            sample[1] = frames[ix + 1];
        }
        return Ok(resampled);
    }

    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn loop_mixer(data: Vec<f32>, sample: usize) -> Mixer {
        let (builder, _) = Mixer::new();
        let mut mixer = builder.build();
        mixer.mixer_state.push(SoundState {
            sound_id: 0,
            play_id: 0,
            sample,
            data: data.into(),
            looped: true,
            volume: 1.0,
            target_volume: 1.0,
            stopping: false,
        });
        mixer
    }
    #[test]
    fn loop_wrap_fills_tail_then_head_without_gaps_or_overlap() {
        let mut m = loop_mixer(vec![0.1, 0.11, 0.2, 0.21, 0.3, 0.31], 4);
        let mut out = [0.0; 8];
        m.fill_audio_buffer(&mut out, 4);
        assert_eq!(out, [0.3, 0.31, 0.1, 0.11, 0.2, 0.21, 0.3, 0.31]);
        m.fill_audio_buffer(&mut out, 4);
        assert_eq!(out, [0.1, 0.11, 0.2, 0.21, 0.3, 0.31, 0.1, 0.11]);
    }
    #[test]
    fn very_short_loops_can_wrap_many_times_in_one_buffer() {
        let mut m = loop_mixer(vec![0.25, 0.5], 0);
        let mut out = [0.0; 16];
        m.fill_audio_buffer(&mut out, 8);
        assert!(out.chunks_exact(2).all(|v| v == [0.25, 0.5]));
    }
    #[test]
    fn exact_buffer_length_transients_end_and_stops_ramp_down() {
        let mut m = loop_mixer(vec![0.4; 8], 0);
        m.mixer_state[0].looped = false;
        let mut out = [0.0; 8];
        m.fill_audio_buffer(&mut out, 4);
        assert!(m.mixer_state.is_empty());
        let mut m = loop_mixer(vec![0.4; 8], 0);
        m.mixer_state[0].target_volume = 0.0;
        m.mixer_state[0].stopping = true;
        m.fill_audio_buffer(&mut out, 4);
        assert!(out[0] > 0.39);
        assert!(out[6] < out[0]);
        let mut tail = vec![0.0; 12000];
        m.fill_audio_buffer(&mut tail, 6000);
        assert!(m.mixer_state.is_empty());
    }

    #[test]
    fn event_storms_are_bounded_and_summed_peaks_cannot_clip() {
        let (builder, control) = Mixer::new();
        let mut mixer = builder.build();
        for sound in 0..16 {
            control.send(AudioMessage::AddSound(sound, vec![0.65; 16000]));
            for voice in 0..8 {
                control.send(AudioMessage::Play(sound, sound * 8 + voice, false, 1.0));
            }
        }
        let mut out = vec![0.0; 4096];
        mixer.fill_audio_buffer(&mut out, 2048);
        assert_eq!(mixer.mixer_state.len(), 32);
        for sound in 0..16 {
            assert!(
                mixer
                    .mixer_state
                    .iter()
                    .filter(|s| s.sound_id == sound)
                    .count()
                    <= 3
            );
        }
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        assert!(out.iter().any(|v| *v > 0.8));
    }

    #[test]
    fn volume_commands_ramp_both_channels_together() {
        let (builder, control) = Mixer::new();
        let mut mixer = builder.build();
        control.send(AudioMessage::AddSound(0, vec![0.4; 8]));
        control.send(AudioMessage::Play(0, 0, true, 1.0));
        let mut out = vec![0.0; 12000];
        mixer.fill_audio_buffer(&mut out, 6000);
        assert!(out[0] < 0.001 && out[out.len() - 1] > 0.399);
        control.send(AudioMessage::SetVolumeAll(0, 0.0));
        mixer.fill_audio_buffer(&mut out, 6000);
        assert!(out[0] > 0.39 && out[out.len() - 1] < 0.00001);
        assert!(out.chunks_exact(2).all(|v| v[0] == v[1]));
        control.send(AudioMessage::StopAll(0));
        mixer.fill_audio_buffer(&mut out, 6000);
        assert!(mixer.mixer_state.is_empty());
    }
}
