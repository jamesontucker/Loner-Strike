//! ls-voice: players' voice to text for the bots (the app's Advanced Bot Interaction).
//!
//! The game server (Loner-Strike's engine, sv_ls_voice_feed) hands every voice packet a player
//! sends to 127.0.0.1:<feed port>. Each player's talking is cut into lines at a short pause,
//! turned into text with Whisper (on this machine: nothing leaves it), tidied, and given to the
//! bots as if typed in team chat, through the server's console (gm_botchat_ask).
//!
//!   ls-voice --feed <port> --rcon-port <port> --rcon-password-file <file> --model <ggml model>
//!            [--threads <n>]

mod clean;
mod rcon;
mod steam;

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Quiet this long and a line is over.
const PAUSE: Duration = Duration::from_millis(600);
/// A line this long is sent on even mid-talk (someone holding the key and rambling).
const LONGEST: Duration = Duration::from_secs(12);
/// Fewer packets than this is a key tap, not speech.
const FEWEST: usize = 4;

struct Args {
    feed: u16,
    rcon_port: u16,
    rcon_password_file: String,
    model: String,
    threads: i32,
}

fn args() -> Args {
    let mut a = Args { feed: 0, rcon_port: 27015, rcon_password_file: String::new(), model: String::new(), threads: 2 };
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i + 1 < v.len() {
        let val = v[i + 1].clone();
        match v[i].as_str() {
            "--feed" => a.feed = val.parse().unwrap_or(0),
            "--rcon-port" => a.rcon_port = val.parse().unwrap_or(27015),
            "--rcon-password-file" => a.rcon_password_file = val,
            "--model" => a.model = val,
            "--threads" => a.threads = val.parse().unwrap_or(2),
            _ => {
                eprintln!("ls-voice: unknown option {}", v[i]);
                std::process::exit(2);
            }
        }
        i += 2;
    }
    if a.feed == 0 || a.rcon_password_file.is_empty() || a.model.is_empty() {
        eprintln!("ls-voice --feed <port> --rcon-port <port> --rcon-password-file <file> --model <ggml model> [--threads <n>]");
        std::process::exit(2);
    }
    a
}

/// One player's talking so far.
struct Talk {
    packets: Vec<Vec<u8>>,
    first: Instant,
    last: Instant,
}

/// A finished line, waiting to be heard.
struct Clip {
    userid: u32,
    packets: Vec<Vec<u8>>,
}

fn main() {
    let a = args();
    let password = match std::fs::read_to_string(&a.rcon_password_file) {
        Ok(p) => p.trim().to_string(),
        Err(e) => {
            eprintln!("ls-voice: can't read the rcon password ({}): {e}", a.rcon_password_file);
            std::process::exit(1);
        }
    };
    let feed = match UdpSocket::bind(("127.0.0.1", a.feed)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ls-voice: can't listen on 127.0.0.1:{}: {e}", a.feed);
            std::process::exit(1);
        }
    };
    feed.set_read_timeout(Some(Duration::from_millis(100))).ok();

    // the hearing: one thread, one line at a time, so voice keeps coming in while it works
    let (tx, rx) = mpsc::channel::<Clip>();
    let (model, threads, rcon_port, pw) = (a.model.clone(), a.threads, a.rcon_port, password.clone());
    let worker = thread::spawn(move || hear(rx, &model, threads, rcon::Rcon::new(rcon_port, pw)));

    let console = rcon::Rcon::new(a.rcon_port, password);
    let mut told = Instant::now() - Duration::from_secs(3600);
    let mut talking: HashMap<u32, Talk> = HashMap::new();
    let mut buf = [0u8; 8192];
    println!("ls-voice: listening on 127.0.0.1:{} for the server on port {}", a.feed, a.rcon_port);
    loop {
        // (the server is told where to send, and again now and then: after a restart it has forgotten)
        if told.elapsed() > Duration::from_secs(30) {
            console.cmd(&format!("sv_ls_voice_feed {}", a.feed));
            told = Instant::now();
        }
        if let Ok((n, from)) = feed.recv_from(&mut buf) {
            // "LSV1", slot, userid (4, little-endian), the packet
            if from.ip().is_loopback() && n > 9 && &buf[..4] == b"LSV1" {
                let userid = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
                let now = Instant::now();
                let t = talking.entry(userid).or_insert_with(|| Talk { packets: Vec::new(), first: now, last: now });
                t.packets.push(buf[9..n].to_vec());
                t.last = now;
            }
        }
        let done: Vec<u32> = talking
            .iter()
            .filter(|(_, t)| t.last.elapsed() > PAUSE || t.first.elapsed() > LONGEST)
            .map(|(u, _)| *u)
            .collect();
        for userid in done {
            if let Some(t) = talking.remove(&userid) {
                if t.packets.len() >= FEWEST && tx.send(Clip { userid, packets: t.packets }).is_err() {
                    eprintln!("ls-voice: the hearing stopped");
                    let _ = worker.join();
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Lines in, text out to the bots.
fn hear(rx: mpsc::Receiver<Clip>, model: &str, threads: i32, console: rcon::Rcon) {
    whisper_rs::install_logging_hooks();
    let ctx = match whisper_rs::WhisperContext::new_with_params(model, whisper_rs::WhisperContextParameters::default()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ls-voice: can't load the model {model}: {e:?}");
            std::process::exit(1);
        }
    };
    let mut state = match ctx.create_state() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ls-voice: can't start the model: {e:?}");
            std::process::exit(1);
        }
    };
    // who's on the server (bot names, the map), looked up now and then
    let mut names: Vec<String> = Vec::new();
    let mut looked = Instant::now() - Duration::from_secs(3600);
    for clip in rx {
        let t0 = Instant::now();
        let pcm = steam::decode(&clip.packets);
        let rate = steam::rate(&clip.packets);
        if pcm.len() < rate as usize / 4 {
            continue;
        }
        let audio = steam::to_16k(&pcm, rate);
        if looked.elapsed() > Duration::from_secs(10) {
            let (bots, _map) = rcon::status(&console.cmd("status"));
            names = clean::name_parts(&bots);
            looked = Instant::now();
        }
        let hint = format!(
            "Counter-Strike team voice. Players: {}. Places: A site, B site, long, short, mid, catwalk, tunnels, ct spawn, t spawn, \
             ramp, connector, banana, apartments, pit, heaven, vents, hostages. AWP, AK, M4, deagle, rotate, plant, defuse, rush, \
             eco, force, save, flash, smoke.",
            names.join(", ")
        );
        let mut p = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        p.set_n_threads(threads);
        p.set_language(Some("en"));
        p.set_no_timestamps(true);
        p.set_single_segment(true);
        p.set_no_context(true);
        p.set_suppress_blank(true);
        p.set_print_special(false);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_timestamps(false);
        p.set_initial_prompt(&hint);
        if let Err(e) = state.full(p, &audio) {
            eprintln!("ls-voice: couldn't hear a line: {e:?}");
            continue;
        }
        let mut heard = String::new();
        for seg in state.as_iter() {
            if let Ok(s) = seg.to_str_lossy() {
                heard.push_str(&s);
            }
        }
        if clean::junk(&heard) {
            continue;
        }
        let line = clean::clean(&heard, &names);
        if line.is_empty() {
            continue;
        }
        println!("ls-voice: #{} ({:.1}s of talk, heard in {} ms): {}", clip.userid, audio.len() as f32 / 16000.0, t0.elapsed().as_millis(), line);
        console.cmd(&format!("gm_botchat_ask {} \"{}\" team", clip.userid, line));
    }
}
