//! Sound, all made here from square, triangle and noise waves: no sound
//! files. One palette for the whole collection, so the eight games sound
//! like one family.

use rodio::buffer::SamplesBuffer;
use rodio::{MixerDeviceSink, Source};
use std::num::NonZero;

const RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sound {
    /// moving between choices
    Tick,
    /// a choice made, a key accepted
    Click,
    /// something refused
    Nope,
    /// a small piece of good news
    Good,
    /// a small piece of bad news
    Bad,
    /// the end of a game, won
    Win,
    /// the end of a game, lost
    Lose,
    /// an explosion or a crash
    Boom,
    /// a rush of air or exhaust, `0.0..=1.0` loud
    Hiss(f32),
    /// a single beep at a pitch, for counters and instruments
    Beep(f32),
}

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Noise,
}

fn note(out: &mut Vec<f32>, wave: Wave, f0: f32, f1: f32, secs: f32, vol: f32) {
    let n = (secs * RATE as f32) as usize;
    let mut phase = 0.0f32;
    let mut seed = 0x9e37_79b9u32;
    for i in 0..n {
        let k = i as f32 / n.max(1) as f32;
        let f = f0 + (f1 - f0) * k;
        phase = (phase + f / RATE as f32).fract();
        let s = match wave {
            Wave::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
            Wave::Noise => {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
            }
        };
        let attack = (i as f32 / (RATE as f32 * 0.003)).min(1.0);
        out.push(s * vol * attack * (1.0 - k));
    }
}

pub struct Audio {
    sink: Option<MixerDeviceSink>,
    pub muted: bool,
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        if muted {
            // never open the device when asked for silence: previews and
            // tests must not reach the speakers at all
            return Audio { sink: None, muted };
        }
        let sink = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut s) => {
                s.log_on_drop(false);
                Some(s)
            }
            Err(e) => {
                log::warn!("no audio: {e}");
                None
            }
        };
        Audio { sink, muted }
    }

    pub fn silent() -> Audio {
        Audio {
            sink: None,
            muted: true,
        }
    }

    pub fn available(&self) -> bool {
        self.sink.is_some()
    }

    /// M turns sound off and on; the device opens the first time it is needed.
    pub fn toggle(&mut self) {
        self.muted = !self.muted;
        if !self.muted && self.sink.is_none() {
            *self = Audio::new(false);
        }
    }

    pub fn play(&self, s: Sound) {
        if self.muted {
            return;
        }
        let Some(sink) = &self.sink else { return };
        let mut v = Vec::new();
        match s {
            Sound::Tick => note(&mut v, Wave::Square, 1320.0, 1320.0, 0.012, 0.05),
            Sound::Click => note(&mut v, Wave::Square, 880.0, 880.0, 0.03, 0.1),
            Sound::Nope => note(&mut v, Wave::Triangle, 150.0, 110.0, 0.1, 0.3),
            Sound::Good => {
                note(&mut v, Wave::Square, 660.0, 660.0, 0.06, 0.12);
                note(&mut v, Wave::Square, 990.0, 990.0, 0.1, 0.12);
            }
            Sound::Bad => {
                note(&mut v, Wave::Triangle, 330.0, 330.0, 0.08, 0.3);
                note(&mut v, Wave::Triangle, 247.0, 220.0, 0.16, 0.3);
            }
            Sound::Win => {
                for f in [523.0, 659.0, 784.0, 1047.0] {
                    note(&mut v, Wave::Square, f, f, 0.1, 0.16);
                }
                note(&mut v, Wave::Square, 1047.0, 1047.0, 0.35, 0.16);
            }
            Sound::Lose => {
                for f in [392.0, 330.0, 262.0] {
                    note(&mut v, Wave::Triangle, f, f, 0.15, 0.3);
                }
                note(&mut v, Wave::Triangle, 196.0, 150.0, 0.5, 0.3);
            }
            Sound::Boom => {
                note(&mut v, Wave::Noise, 0.0, 0.0, 0.6, 0.45);
                note(&mut v, Wave::Triangle, 90.0, 40.0, 0.4, 0.4);
            }
            Sound::Hiss(loud) => note(
                &mut v,
                Wave::Noise,
                0.0,
                0.0,
                0.12,
                0.05 + 0.2 * loud.clamp(0.0, 1.0),
            ),
            Sound::Beep(f) => note(&mut v, Wave::Square, f, f, 0.04, 0.08),
        }
        let buf = SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(RATE).unwrap(), v);
        sink.mixer().add(buf.amplify(0.8));
    }
}
