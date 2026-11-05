//! Sound effects, with no crates and no sound files.
//!
//! Every effect is a short recipe of chiptune tones (square, triangle and
//! noise voices with pitch sweeps). A background thread mixes the voices
//! that are playing and streams raw 16-bit mono PCM into the first system
//! player that works (`pw-cat`, `pacat`, then `aplay`). If none does, the
//! game stays silent. `game.rs` only queues `Sfx` values (`Game::sounds`);
//! `main.rs` hands them to `Audio::play`.

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

/// Samples per second of the stream.
pub const RATE: u32 = 32_000;
/// Samples mixed per write: 8 ms.
const CHUNK: usize = 256;
/// How far the mixer runs ahead of the wall clock. Short, so a key press
/// is heard almost at once, but long enough not to stutter.
const LEAD: f64 = 0.035;
/// Voices beyond this are dropped (oldest first) so bursts stay clean.
const MAX_VOICES: usize = 24;

/// Every sound the game makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    /// The selection moved in a menu or the shop.
    Hover,
    /// A menu choice was made.
    Select,
    /// The pause menu opened.
    Pause,
    /// A correct letter fired a shot; the number is the letter's position,
    /// so a word climbs in pitch as it is typed.
    Fire(u8),
    /// A wrong key.
    Typo,
    /// A hazard broke apart; `big` for long words.
    Break {
        big: bool,
    },
    /// A word typed with no mistakes.
    Perfect,
    /// Crystals reached the ship.
    Gem,
    /// A card was bought (or sold).
    Buy,
    /// Not allowed: too few crystals, sold out, rack full.
    Deny,
    Reroll,
    /// A heart was lost.
    Hurt,
    /// "GO!": a wave begins.
    WaveStart,
    /// "CLEAR!": a wave is done.
    WaveClear,
    /// A boss arrives.
    BossWarning,
    /// A boss word landed.
    BossHit,
    /// The boss is beaten.
    BossDown,
    GameOver,
    Overdrive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wave {
    /// Square with a duty cycle (0.5 is hollow, 0.125 is thin and buzzy).
    Pulse(f32),
    Triangle,
    /// Sample-and-hold noise; the tone's frequency is how often it changes.
    Noise,
}

/// One voice of a recipe: starts `at` seconds in, lasts `dur`, sweeps
/// exponentially from `f0` to `f1` Hz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    pub at: f32,
    pub dur: f32,
    pub f0: f32,
    pub f1: f32,
    pub wave: Wave,
    pub vol: f32,
}

const fn tone(at: f32, dur: f32, f0: f32, f1: f32, wave: Wave, vol: f32) -> Tone {
    Tone {
        at,
        dur,
        f0,
        f1,
        wave,
        vol,
    }
}

const SQ: Wave = Wave::Pulse(0.5);
const THIN: Wave = Wave::Pulse(0.25);
const TRI: Wave = Wave::Triangle;
const NOISE: Wave = Wave::Noise;

/// A rising run of notes, `step` seconds apart.
fn arpeggio(notes: &[f32], step: f32, dur: f32, wave: Wave, vol: f32) -> Vec<Tone> {
    notes
        .iter()
        .enumerate()
        .map(|(i, &f)| tone(i as f32 * step, dur, f, f, wave, vol))
        .collect()
}

/// The tones that make up `sfx`.
pub fn recipe(sfx: Sfx) -> Vec<Tone> {
    match sfx {
        Sfx::Hover => vec![tone(0.0, 0.025, 1900.0, 1700.0, TRI, 0.22)],
        Sfx::Select => vec![
            tone(0.0, 0.045, 660.0, 660.0, THIN, 0.16),
            tone(0.045, 0.07, 990.0, 990.0, THIN, 0.16),
        ],
        Sfx::Pause => vec![
            tone(0.0, 0.06, 784.0, 784.0, TRI, 0.3),
            tone(0.06, 0.09, 523.0, 523.0, TRI, 0.3),
        ],
        Sfx::Fire(i) => {
            // A short laser chirp that climbs a semitone per letter.
            let f = 1300.0 * 2f32.powf(i.min(12) as f32 / 12.0);
            vec![tone(0.0, 0.06, f, f * 0.35, THIN, 0.07)]
        }
        Sfx::Typo => vec![tone(0.0, 0.09, 150.0, 110.0, Wave::Pulse(0.125), 0.16)],
        Sfx::Break { big } => {
            let k = if big { 1.6 } else { 1.0 };
            vec![
                tone(0.0, 0.2 * k, 5000.0, 400.0, NOISE, 0.22),
                tone(0.0, 0.14 * k, 160.0, 45.0, TRI, 0.45),
            ]
        }
        Sfx::Perfect => arpeggio(&[1047.0, 1319.0, 1568.0, 2093.0], 0.035, 0.07, TRI, 0.22),
        Sfx::Gem => vec![tone(0.0, 0.04, 2637.0, 3136.0, TRI, 0.1)],
        Sfx::Buy => vec![
            tone(0.0, 0.05, 1319.0, 1319.0, SQ, 0.1),
            tone(0.05, 0.16, 1760.0, 1760.0, SQ, 0.1),
            tone(0.05, 0.16, 2637.0, 2637.0, TRI, 0.12),
        ],
        Sfx::Deny => vec![
            tone(0.0, 0.07, 196.0, 196.0, SQ, 0.12),
            tone(0.09, 0.1, 147.0, 147.0, SQ, 0.12),
        ],
        Sfx::Reroll => vec![
            tone(0.0, 0.12, 300.0, 1400.0, THIN, 0.09),
            tone(0.0, 0.1, 2000.0, 6000.0, NOISE, 0.06),
        ],
        Sfx::Hurt => vec![
            tone(0.0, 0.3, 440.0, 70.0, SQ, 0.18),
            tone(0.0, 0.25, 3000.0, 200.0, NOISE, 0.16),
        ],
        Sfx::WaveStart => vec![
            tone(0.0, 0.08, 523.0, 523.0, SQ, 0.1),
            tone(0.09, 0.16, 1047.0, 1047.0, SQ, 0.1),
        ],
        Sfx::WaveClear => arpeggio(&[523.0, 659.0, 784.0, 1047.0, 1319.0], 0.06, 0.12, SQ, 0.09),
        Sfx::BossWarning => (0..6)
            .map(|i| {
                let f = if i % 2 == 0 { 880.0 } else { 660.0 };
                tone(i as f32 * 0.13, 0.12, f, f, SQ, 0.11)
            })
            .collect(),
        Sfx::BossHit => vec![
            tone(0.0, 0.1, 3000.0, 600.0, NOISE, 0.16),
            tone(0.0, 0.08, 220.0, 90.0, TRI, 0.35),
        ],
        Sfx::BossDown => {
            let mut v = vec![
                tone(0.0, 0.6, 3000.0, 120.0, NOISE, 0.3),
                tone(0.0, 0.45, 120.0, 35.0, TRI, 0.5),
            ];
            v.extend(
                arpeggio(&[523.0, 659.0, 784.0, 1047.0], 0.08, 0.14, SQ, 0.09)
                    .into_iter()
                    .map(|t| Tone {
                        at: t.at + 0.35,
                        ..t
                    }),
            );
            v
        }
        Sfx::GameOver => arpeggio(&[392.0, 330.0, 262.0, 196.0], 0.18, 0.2, SQ, 0.12),
        Sfx::Overdrive => vec![
            tone(0.0, 0.4, 180.0, 1600.0, THIN, 0.1),
            tone(0.0, 0.4, 90.0, 800.0, TRI, 0.25),
        ],
    }
}

/// One tone being played.
struct Voice {
    tone: Tone,
    /// Seconds since it started; negative while it waits for its turn.
    t: f32,
    phase: f32,
    held: f32,
    seed: u32,
}

impl Voice {
    fn new(tone: Tone, delay: f32, seed: u32) -> Self {
        Voice {
            tone,
            t: -(tone.at + delay),
            phase: 0.0,
            held: 0.0,
            seed: seed | 1,
        }
    }

    fn done(&self) -> bool {
        self.t >= self.tone.dur
    }

    /// The next sample, in -1..1 before volume.
    fn sample(&mut self) -> f32 {
        let dt = 1.0 / RATE as f32;
        let t = self.t;
        self.t += dt;
        if t < 0.0 || t >= self.tone.dur {
            return 0.0;
        }
        let Tone {
            dur, f0, f1, wave, ..
        } = self.tone;
        let freq = f0 * (f1 / f0).powf(t / dur);
        let before = self.phase;
        self.phase = (self.phase + freq * dt).fract();
        let raw = match wave {
            Wave::Pulse(duty) => {
                if self.phase < duty {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            Wave::Noise => {
                if self.phase < before {
                    // xorshift: a new random level each cycle.
                    self.seed ^= self.seed << 13;
                    self.seed ^= self.seed >> 17;
                    self.seed ^= self.seed << 5;
                    self.held = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
                }
                self.held
            }
        };
        // 3 ms attack, then a curved fade to silence.
        let attack = (t / 0.003).min(1.0);
        let fade = 1.0 - t / dur;
        raw * attack * fade * fade * self.tone.vol
    }
}

/// All voices playing, mixed into one stream.
pub struct Mixer {
    voices: Vec<Voice>,
    seed: u32,
    smooth: f32,
}

impl Default for Mixer {
    fn default() -> Self {
        Mixer {
            voices: Vec::new(),
            seed: 0x9E37_79B9,
            smooth: 0.0,
        }
    }
}

impl Mixer {
    /// Start `sfx` after `delay` seconds at `volume` (0 to 1).
    pub fn trigger(&mut self, sfx: Sfx, delay: f32, volume: f32) {
        for mut tone in recipe(sfx) {
            tone.vol *= volume;
            self.seed = self
                .seed
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            self.voices.push(Voice::new(tone, delay, self.seed));
        }
        if self.voices.len() > MAX_VOICES {
            let extra = self.voices.len() - MAX_VOICES;
            self.voices.drain(..extra);
        }
    }

    #[cfg(test)]
    pub fn idle(&self) -> bool {
        self.voices.is_empty()
    }

    /// Fill `out` with the next samples.
    pub fn render(&mut self, out: &mut [i16]) {
        for s in out.iter_mut() {
            let mut x: f32 = self.voices.iter_mut().map(Voice::sample).sum();
            // Soft clip (a tanh curve), then a gentle low-pass to take the
            // edge off the square waves.
            x = x * (27.0 + x * x) / (27.0 + 9.0 * x * x);
            self.smooth += 0.55 * (x - self.smooth);
            *s = (self.smooth.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        }
        self.voices.retain(|v| !v.done());
    }
}

/// Players to try, in order, each reading raw mono s16 PCM on stdin.
fn players() -> [Vec<String>; 3] {
    let rate = RATE.to_string();
    let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    [
        args(&[
            "pw-cat",
            "--playback",
            "--raw",
            "--rate",
            &rate,
            "--channels",
            "1",
            "--format",
            "s16",
            "--latency",
            "15ms",
            "-",
        ]),
        args(&[
            "pacat",
            "--playback",
            "--raw",
            &format!("--rate={rate}"),
            "--channels=1",
            "--format=s16le",
            "--latency-msec=25",
        ]),
        args(&[
            "aplay",
            "-q",
            "-t",
            "raw",
            "-f",
            "S16_LE",
            "-r",
            &rate,
            "-c",
            "1",
            "--buffer-time=40000",
        ]),
    ]
}

fn spawn(cmd: &[String]) -> Option<Child> {
    Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// A sound request: what, after how long, how loud.
type Request = (Sfx, f32, f32);

/// The game's handle on the sound thread. Dropping it stops the sound.
pub struct Audio {
    tx: Option<Sender<Request>>,
}

impl Audio {
    /// Start the mixer thread. Never fails: without a player it is silent.
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let ok = thread::Builder::new()
            .name("typoids-audio".into())
            .spawn(move || run(rx))
            .is_ok();
        Audio {
            tx: ok.then_some(tx),
        }
    }

    pub fn play(&self, sfx: Sfx, delay: f32, volume: f32) {
        if volume <= 0.0 {
            return;
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send((sfx, delay, volume));
        }
    }
}

/// The mixer thread: find a player, then stream until the game quits.
fn run(rx: Receiver<Request>) {
    for cmd in players() {
        let Some(mut child) = spawn(&cmd) else {
            continue;
        };
        let started = Instant::now();
        let streamed = stream(&mut child, &rx);
        let _ = child.kill();
        let _ = child.wait();
        // A player that dies at once is missing a server: try the next.
        // One that ran a while and stopped means the game is quitting.
        if streamed || started.elapsed() > Duration::from_secs(1) {
            return;
        }
    }
}

/// Feed the player, paced to the wall clock. Returns true when the game
/// hung up (normal exit), false when the player stopped taking samples.
fn stream(child: &mut Child, rx: &Receiver<Request>) -> bool {
    let Some(mut pipe) = child.stdin.take() else {
        return false;
    };
    let mut mixer = Mixer::default();
    let mut buf = [0i16; CHUNK];
    let mut bytes = [0u8; CHUNK * 2];
    let start = Instant::now();
    let mut written: u64 = 0;
    loop {
        loop {
            match rx.try_recv() {
                Ok((sfx, delay, volume)) => mixer.trigger(sfx, delay, volume),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
        let ahead = written as f64 / RATE as f64 - start.elapsed().as_secs_f64();
        if ahead > LEAD {
            thread::sleep(Duration::from_secs_f64((ahead - LEAD).min(0.02)));
            continue;
        }
        mixer.render(&mut buf);
        for (i, s) in buf.iter().enumerate() {
            bytes[i * 2..i * 2 + 2].copy_from_slice(&s.to_le_bytes());
        }
        if pipe.write_all(&bytes).is_err() {
            return false;
        }
        written += CHUNK as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Sfx; 20] = [
        Sfx::Hover,
        Sfx::Select,
        Sfx::Pause,
        Sfx::Fire(0),
        Sfx::Typo,
        Sfx::Break { big: false },
        Sfx::Break { big: true },
        Sfx::Perfect,
        Sfx::Gem,
        Sfx::Buy,
        Sfx::Deny,
        Sfx::Reroll,
        Sfx::Hurt,
        Sfx::WaveStart,
        Sfx::WaveClear,
        Sfx::BossWarning,
        Sfx::BossHit,
        Sfx::BossDown,
        Sfx::GameOver,
        Sfx::Overdrive,
    ];

    #[test]
    fn every_effect_is_short_audible_and_ends() {
        for sfx in ALL {
            let tones = recipe(sfx);
            assert!(!tones.is_empty(), "{sfx:?}");
            let end = tones.iter().map(|t| t.at + t.dur).fold(0.0, f32::max);
            assert!(end > 0.0 && end < 1.5, "{sfx:?} lasts {end}s");
            let mut m = Mixer::default();
            m.trigger(sfx, 0.0, 1.0);
            let mut out = vec![0i16; (end * RATE as f32) as usize + CHUNK];
            m.render(&mut out);
            let peak = out.iter().map(|s| s.unsigned_abs()).max().unwrap();
            assert!(peak > 500, "{sfx:?} is silent");
            assert!(m.idle(), "{sfx:?} keeps playing");
        }
    }

    #[test]
    fn a_burst_of_sounds_never_clips_or_piles_up() {
        let mut m = Mixer::default();
        for sfx in ALL {
            m.trigger(sfx, 0.0, 1.0);
        }
        assert!(m.voices.len() <= MAX_VOICES);
        let mut out = vec![0i16; RATE as usize / 4];
        m.render(&mut out);
        // The soft clip keeps the stack below full scale.
        assert!(out.iter().all(|s| s.unsigned_abs() < i16::MAX as u16));
    }

    #[test]
    fn typed_words_climb_in_pitch_and_delays_wait() {
        assert!(recipe(Sfx::Fire(4))[0].f0 > recipe(Sfx::Fire(0))[0].f0);
        let mut m = Mixer::default();
        m.trigger(Sfx::Hover, 0.1, 1.0);
        let mut out = vec![0i16; (RATE as f32 * 0.09) as usize];
        m.render(&mut out);
        assert!(out.iter().all(|&s| s.abs() < 50), "silent until its delay");
    }
}
