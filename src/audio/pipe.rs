//! Linux output: stream raw mono 16-bit PCM into a system player.
//! PipeWire, PulseAudio and plain ALSA all ship one, so no crate is needed.

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use super::{Mixer, Request, RATE};

/// Samples mixed per write: 8 ms.
const CHUNK: usize = 256;
/// How far the mixer runs ahead of the wall clock. Short, so a key press
/// is heard almost at once, but long enough not to stutter.
const LEAD: f64 = 0.035;

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

/// The mixer thread: find a player, then stream until the game quits.
pub fn run(rx: Receiver<Request>) {
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
    let mut mixer = Mixer::new(RATE);
    let mut buf = [0i16; CHUNK];
    let mut bytes = [0u8; CHUNK * 2];
    let start = Instant::now();
    let mut written: u64 = 0;
    loop {
        if !mixer.take_requests(rx) {
            return true;
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
