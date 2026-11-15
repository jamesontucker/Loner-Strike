//! Sounds brought into the game (the Content Browser's Import, the asset pickers' Import): WAV,
//! MP3, Ogg Vorbis and FLAC in, a WAV the engine plays well out. PCM 16-bit (or 8-bit), 22050 Hz
//! by default (11025 and 44100 too), mono unless the sound is "2D only" (then stereo stays),
//! peak-normalized, silence at the ends optionally trimmed, and a WAV's loop points (its cue
//! points, their lengths, and a sampler loop) kept at the new rate. Decoding is Symphonia
//! (pure Rust, MPL-2.0); resampling is a windowed-sinc filter here. Never a program on the
//! computer.
//!
//! A WAV that already is what was asked for (PCM at that rate, bit depth and channel count, no
//! trimming) is left exactly as it is.

use serde::Deserialize;

/// What the converted sound should be (`sound` in an import request).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(default)]
pub struct SoundOpts {
    /// 11025, 22050 or 44100
    pub rate: u32,
    /// 8 or 16
    pub bits: u8,
    /// "2D only": a stereo sound stays stereo (the engine plays stereo only without a position)
    pub stereo: bool,
    /// trim near-silence from the start and the end
    pub trim: bool,
}

impl Default for SoundOpts {
    fn default() -> Self {
        SoundOpts { rate: 22050, bits: 16, stereo: false, trim: false }
    }
}

impl SoundOpts {
    /// The options with anything out of range set back to the default.
    pub fn clean(self) -> SoundOpts {
        SoundOpts {
            rate: if [11025, 22050, 44100].contains(&self.rate) { self.rate } else { 22050 },
            bits: if self.bits == 8 { 8 } else { 16 },
            ..self
        }
    }
}

/// At most this long (the engine streams nothing: a sound is loaded whole).
const MAX_SECONDS: f64 = 300.0;
/// Input and output size caps (the importer's own cap for a .wav).
const MAX_BYTES: usize = 32 << 20;

/// The sound formats an import takes, by extension (lower case), with their names for people.
pub fn sound_input(name: &str) -> Option<(&'static str, &'static str)> {
    let low = name.to_ascii_lowercase();
    match low.rsplit_once('.').map(|x| x.1)? {
        "wav" => Some(("wav", "WAV")),
        "mp3" => Some(("mp3", "MP3")),
        "ogg" | "oga" => Some(("ogg", "Ogg")),
        "flac" => Some(("flac", "FLAC")),
        _ => None,
    }
}

/// A file ready to import: its name (a converted sound is `<stem>.wav`), its bytes, and what the
/// conversion did ("converted from 44.1 kHz stereo MP3 to 22 kHz mono WAV"), if anything.
#[derive(Debug)]
pub struct Prepared {
    pub name: String,
    pub bytes: Vec<u8>,
    pub summary: Option<String>,
}

/// Anything that isn't a sound passes as it is; a sound is checked and converted as asked.
pub fn prepare(name: &str, bytes: Vec<u8>, opts: SoundOpts) -> Result<Prepared, String> {
    let Some((ext, label)) = sound_input(name) else { return Ok(Prepared { name: name.to_string(), bytes, summary: None }) };
    let opts = opts.clean();
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = &file[..file.rfind('.').unwrap_or(file.len())];
    let out_name = format!("{stem}.wav");
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(format!("{file} is empty or bigger than {} MB", MAX_BYTES >> 20));
    }
    let wav = if ext == "wav" { parse_wav(&bytes) } else { None };
    if ext == "wav" && wav.is_none() {
        return Err(format!("{file} isn't really a WAV file (its first bytes say otherwise)"));
    }
    // (a WAV that already is what was asked for stays exactly as it is)
    if let Some(w) = &wav {
        if w.format == 1 && w.bits == opts.bits as u16 && w.rate == opts.rate && (w.channels == 1 || (w.channels == 2 && opts.stereo)) && !opts.trim {
            return Ok(Prepared { name: out_name, bytes, summary: None });
        }
    }
    let d = decode(&bytes, ext).map_err(|e| format!("{file} couldn't be read as {label} ({e})"))?;
    if d.channels.is_empty() || d.channels[0].is_empty() || d.rate == 0 {
        return Err(format!("{file} has no sound in it"));
    }
    if d.channels[0].len() as f64 / d.rate as f64 > MAX_SECONDS {
        return Err(format!("{file} is longer than {} minutes", MAX_SECONDS as u32 / 60));
    }
    let in_channels = d.channels.len();
    // channels: mono (the average), or the first two for "2D only"
    let chans: Vec<Vec<f32>> = if opts.stereo && in_channels >= 2 {
        d.channels[..2].to_vec()
    } else {
        let n = d.channels[0].len();
        let mut m = vec![0f32; n];
        for c in &d.channels {
            for (i, v) in c.iter().enumerate().take(n) {
                m[i] += v / in_channels as f32;
            }
        }
        vec![m]
    };
    let mut chans: Vec<Vec<f32>> = chans.iter().map(|c| resample(c, d.rate, opts.rate)).collect();
    // trim near-silence at the ends (keeping 10 ms), loop points moved with it
    let mut lead = 0usize;
    let mut trimmed = false;
    if opts.trim {
        let len = chans[0].len();
        let loud = |i: usize| chans.iter().any(|c| c[i].abs() > 0.004);
        let first = (0..len).find(|&i| loud(i));
        if let Some(first) = first {
            let last = (0..len).rev().find(|&i| loud(i)).unwrap_or(len - 1);
            let pad = (opts.rate / 100) as usize;
            lead = first.saturating_sub(pad);
            let end = (last + pad + 1).min(len);
            trimmed = lead > 0 || end < len;
            for c in chans.iter_mut() {
                *c = c[lead..end].to_vec();
            }
        }
    }
    // normalize the peak just under full scale
    let peak = chans.iter().flat_map(|c| c.iter()).fold(0f32, |m, v| m.max(v.abs()));
    if peak > 1e-6 {
        let g = 0.98 / peak;
        for c in chans.iter_mut() {
            for v in c.iter_mut() {
                *v *= g;
            }
        }
    }
    // loop points at the new rate
    let len = chans[0].len() as u32;
    let scale = |p: u32| -> u32 {
        let v = (p as f64 * opts.rate as f64 / d.rate as f64).round() as i64 - lead as i64;
        v.clamp(0, len.saturating_sub(1) as i64) as u32
    };
    let marks = wav.as_ref().map(|w| Marks {
        cues: w.marks.cues.iter().map(|&(id, p)| (id, scale(p))).collect(),
        lengths: w.marks.lengths.iter().map(|&(id, l)| (id, (l as f64 * opts.rate as f64 / d.rate as f64).round() as u32)).collect(),
        loops: w.marks.loops.iter().map(|&(a, b)| (scale(a), scale(b))).collect(),
    });
    let out = write_wav(&chans, opts.rate, opts.bits, marks.as_ref());
    if out.len() > MAX_BYTES {
        return Err(format!("{file} would be bigger than {} MB as a WAV: pick a lower rate, mono or 8-bit", MAX_BYTES >> 20));
    }
    let mut summary = format!(
        "converted from {} {} {label} to {} {} WAV",
        rate_words(d.rate),
        channel_words(in_channels),
        rate_words(opts.rate),
        channel_words(chans.len())
    );
    if opts.bits == 8 {
        summary.push_str(", 8-bit");
    }
    if trimmed {
        summary.push_str(", silence trimmed");
    }
    if marks.as_ref().is_some_and(|m| !m.cues.is_empty() || !m.loops.is_empty()) {
        summary.push_str(", loop points kept");
    }
    Ok(Prepared { name: out_name, bytes: out, summary: Some(summary) })
}

fn rate_words(r: u32) -> String {
    match r {
        44100 => "44.1 kHz".into(),
        22050 => "22 kHz".into(),
        11025 => "11 kHz".into(),
        r if r % 1000 == 0 => format!("{} kHz", r / 1000),
        r => format!("{:.1} kHz", r as f64 / 1000.0),
    }
}

fn channel_words(n: usize) -> String {
    match n {
        1 => "mono".into(),
        2 => "stereo".into(),
        n => format!("{n}-channel"),
    }
}

// ---- decoding (Symphonia) -------------------------------------------------------------------

struct Decoded {
    rate: u32,
    /// samples per channel, -1..1
    channels: Vec<Vec<f32>>,
}

fn decode(bytes: &[u8], ext: &str) -> Result<Decoded, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);
    let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()).map_err(|_| "not a sound it knows".to_string())?;
    let mut format = probed.format;
    let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).ok_or("no sound in it")?;
    let id = track.id;
    let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|_| "a kind of sound it can't decode".to_string())?;
    let mut rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut channels: Vec<Vec<f32>> = Vec::new();
    let max_frames = (MAX_SECONDS * 192_000.0) as usize;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(_) if !channels.is_empty() => break,
            Err(e) => return Err(format!("{e}")),
        };
        if packet.track_id() != id {
            continue;
        }
        let buf = match dec.decode(&packet) {
            Ok(b) => b,
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("{e}")),
        };
        let spec = *buf.spec();
        if rate == 0 {
            rate = spec.rate;
        }
        let n = spec.channels.count().max(1);
        if channels.is_empty() {
            channels = vec![Vec::new(); n.min(8)];
        }
        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
        sb.copy_interleaved_ref(buf);
        for frame in sb.samples().chunks(n) {
            for (c, v) in channels.iter_mut().zip(frame) {
                c.push(v.clamp(-1.0, 1.0));
            }
        }
        if channels[0].len() > max_frames {
            return Err("it's too long".into());
        }
    }
    Ok(Decoded { rate, channels })
}

// ---- resampling: a windowed-sinc (Blackman) low-pass interpolator ---------------------------------

/// `x` at `from` Hz, as samples at `to` Hz. Down-sampling filters at the new Nyquist rate first.
pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || x.is_empty() {
        return x.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let n_out = ((x.len() as f64) * ratio).round().max(1.0) as usize;
    let fc = ratio.min(1.0);
    // 16 zero crossings each side, at the filter's own cutoff
    let half = 16.0 / fc;
    let mut out = Vec::with_capacity(n_out);
    let len = x.len() as i64;
    for j in 0..n_out {
        let t = j as f64 / ratio;
        let lo = (t - half).ceil() as i64;
        let hi = (t + half).floor() as i64;
        let mut s = 0f64;
        for k in lo.max(0)..=hi.min(len - 1) {
            let d = t - k as f64;
            let u = d / half;
            let w = 0.42 + 0.5 * (std::f64::consts::PI * u).cos() + 0.08 * (2.0 * std::f64::consts::PI * u).cos();
            let a = std::f64::consts::PI * fc * d;
            let sinc = if a.abs() < 1e-9 { 1.0 } else { a.sin() / a };
            s += x[k as usize] as f64 * fc * sinc * w;
        }
        out.push(s as f32);
    }
    out
}

// ---- WAV reading (for loop points and "already fine") and writing --------------------------------

/// A WAV's loop marks: cue points (id, sample), their lengths from a LIST/adtl "ltxt" (id,
/// samples), and sampler loops (start, end).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Marks {
    pub cues: Vec<(u32, u32)>,
    pub lengths: Vec<(u32, u32)>,
    pub loops: Vec<(u32, u32)>,
}

#[derive(Debug)]
pub struct WavInfo {
    pub format: u16,
    pub channels: u16,
    pub rate: u32,
    pub bits: u16,
    pub frames: u32,
    pub marks: Marks,
}

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The WAV's format and loop marks (None: not a RIFF/WAVE with a fmt chunk).
pub fn parse_wav(b: &[u8]) -> Option<WavInfo> {
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let mut at = 12usize;
    let mut fmt = None;
    let mut data_len = 0u32;
    let mut marks = Marks::default();
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let size = u32le(b, at + 4) as usize;
        let body = at + 8;
        let end = body.saturating_add(size).min(b.len());
        let c = &b[body..end];
        match id {
            b"fmt " if c.len() >= 16 => fmt = Some((u16le(c, 0), u16le(c, 2), u32le(c, 4), u16le(c, 14))),
            b"data" => data_len = size as u32,
            b"cue " if c.len() >= 4 => {
                let n = (u32le(c, 0) as usize).min(256);
                for i in 0..n {
                    let o = 4 + i * 24;
                    if o + 24 <= c.len() {
                        marks.cues.push((u32le(c, o), u32le(c, o + 20)));
                    }
                }
            }
            b"LIST" if c.len() >= 4 && &c[..4] == b"adtl" => {
                let mut s = 4usize;
                while s + 8 <= c.len() {
                    let sid = &c[s..s + 4];
                    let ssize = u32le(c, s + 4) as usize;
                    if sid == b"ltxt" && s + 8 + 8 <= c.len() {
                        marks.lengths.push((u32le(c, s + 8), u32le(c, s + 12)));
                    }
                    s = s + 8 + ssize + (ssize & 1);
                }
            }
            b"smpl" if c.len() >= 36 => {
                let n = (u32le(c, 28) as usize).min(64);
                for i in 0..n {
                    let o = 36 + i * 24;
                    if o + 24 <= c.len() {
                        marks.loops.push((u32le(c, o + 8), u32le(c, o + 12)));
                    }
                }
            }
            _ => {}
        }
        at = body.saturating_add(size).saturating_add(size & 1);
    }
    let (format, channels, rate, bits) = fmt?;
    let frame = (channels as u32 * bits as u32 / 8).max(1);
    Some(WavInfo { format, channels, rate, bits, frames: data_len / frame, marks })
}

/// A PCM WAV (8-bit unsigned or 16-bit signed), with cue points, their lengths and sampler loops
/// when there are any.
pub fn write_wav(chans: &[Vec<f32>], rate: u32, bits: u8, marks: Option<&Marks>) -> Vec<u8> {
    let nch = chans.len().max(1) as u16;
    let frames = chans.first().map_or(0, |c| c.len());
    let bytes_per = if bits == 8 { 1 } else { 2 };
    let mut data = Vec::with_capacity(frames * nch as usize * bytes_per);
    for i in 0..frames {
        for c in chans {
            let v = c[i].clamp(-1.0, 1.0);
            if bits == 8 {
                data.push((v * 127.0 + 128.0).round().clamp(0.0, 255.0) as u8);
            } else {
                data.extend_from_slice(&((v * 32767.0).round() as i16).to_le_bytes());
            }
        }
    }
    let mut chunks: Vec<u8> = Vec::new();
    let mut chunk = |id: &[u8; 4], body: &[u8]| {
        chunks.extend_from_slice(id);
        chunks.extend_from_slice(&(body.len() as u32).to_le_bytes());
        chunks.extend_from_slice(body);
        if body.len() & 1 == 1 {
            chunks.push(0);
        }
    };
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&nch.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * nch as u32 * bytes_per as u32).to_le_bytes());
    fmt.extend_from_slice(&(nch * bytes_per as u16).to_le_bytes());
    fmt.extend_from_slice(&(bits as u16).to_le_bytes());
    chunk(b"fmt ", &fmt);
    chunk(b"data", &data);
    if let Some(m) = marks {
        if !m.cues.is_empty() {
            let mut c = Vec::new();
            c.extend_from_slice(&(m.cues.len() as u32).to_le_bytes());
            for &(id, pos) in &m.cues {
                c.extend_from_slice(&id.to_le_bytes());
                c.extend_from_slice(&pos.to_le_bytes());
                c.extend_from_slice(b"data");
                c.extend_from_slice(&0u32.to_le_bytes());
                c.extend_from_slice(&0u32.to_le_bytes());
                c.extend_from_slice(&pos.to_le_bytes());
            }
            chunk(b"cue ", &c);
        }
        if !m.lengths.is_empty() {
            let mut c = b"adtl".to_vec();
            for &(id, len) in &m.lengths {
                c.extend_from_slice(b"ltxt");
                c.extend_from_slice(&20u32.to_le_bytes());
                c.extend_from_slice(&id.to_le_bytes());
                c.extend_from_slice(&len.to_le_bytes());
                c.extend_from_slice(b"mark");
                c.extend_from_slice(&[0u8; 8]);
            }
            chunk(b"LIST", &c);
        }
        if !m.loops.is_empty() {
            let mut c = vec![0u8; 36];
            c[8..12].copy_from_slice(&(1_000_000_000u32 / rate.max(1)).to_le_bytes());
            c[12..16].copy_from_slice(&60u32.to_le_bytes());
            c[28..32].copy_from_slice(&(m.loops.len() as u32).to_le_bytes());
            for (i, &(a, b)) in m.loops.iter().enumerate() {
                c.extend_from_slice(&(i as u32).to_le_bytes());
                c.extend_from_slice(&0u32.to_le_bytes());
                c.extend_from_slice(&a.to_le_bytes());
                c.extend_from_slice(&b.to_le_bytes());
                c.extend_from_slice(&0u32.to_le_bytes());
                c.extend_from_slice(&0u32.to_le_bytes());
            }
            chunk(b"smpl", &c);
        }
    }
    let mut out = Vec::with_capacity(12 + chunks.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(4 + chunks.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(&chunks);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MP3: &[u8] = include_bytes!("testdata/sound/tone.mp3");
    const OGG: &[u8] = include_bytes!("testdata/sound/tone.ogg");
    const FLAC: &[u8] = include_bytes!("testdata/sound/tone.flac");

    fn tone(rate: u32, secs: f32, ch: usize, amp: f32) -> Vec<Vec<f32>> {
        let n = (rate as f32 * secs) as usize;
        (0..ch).map(|c| (0..n).map(|i| amp * (2.0 * std::f32::consts::PI * (440.0 + 220.0 * c as f32) * i as f32 / rate as f32).sin()).collect()).collect()
    }

    fn samples16(w: &[u8]) -> Vec<i16> {
        let info = parse_wav(w).unwrap();
        assert_eq!(info.bits, 16);
        let at = w.windows(4).position(|x| x == b"data").unwrap() + 8;
        w[at..at + info.frames as usize * info.channels as usize * 2].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
    }

    #[test]
    fn each_format_becomes_an_engine_wav() {
        for (name, bytes, from) in [("tone.mp3", MP3, "converted from 44.1 kHz stereo MP3 to 22 kHz mono WAV"), ("tone.ogg", OGG, "converted from 44.1 kHz stereo Ogg to 22 kHz mono WAV"), ("tone.flac", FLAC, "converted from 22 kHz mono FLAC to 22 kHz mono WAV")] {
            let p = prepare(name, bytes.to_vec(), SoundOpts::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(p.name.ends_with(".wav") && p.name.starts_with("tone"), "{}", p.name);
            let w = parse_wav(&p.bytes).unwrap();
            assert_eq!((w.format, w.channels, w.rate, w.bits), (1, 1, 22050, 16), "{name}");
            assert!(w.frames > 3000, "{name}: {} frames", w.frames);
            assert_eq!(p.summary.as_deref(), Some(from));
            // normalized: the loudest sample just under full scale
            let peak = samples16(&p.bytes).iter().map(|v| v.unsigned_abs()).max().unwrap();
            assert!((31000..=32767).contains(&peak), "{name}: peak {peak}");
        }
    }

    #[test]
    fn options_rate_bits_and_2d_stereo() {
        let p = prepare("tone.mp3", MP3.to_vec(), SoundOpts { rate: 11025, bits: 8, stereo: true, trim: false }).unwrap();
        let w = parse_wav(&p.bytes).unwrap();
        assert_eq!((w.channels, w.rate, w.bits), (2, 11025, 8));
        assert_eq!(p.summary.as_deref(), Some("converted from 44.1 kHz stereo MP3 to 11 kHz stereo WAV, 8-bit"));
        let p = prepare("tone.flac", FLAC.to_vec(), SoundOpts { rate: 44100, ..Default::default() }).unwrap();
        let w = parse_wav(&p.bytes).unwrap();
        assert_eq!((w.channels, w.rate), (1, 44100));
        // out-of-range options fall back to the defaults
        assert_eq!(SoundOpts { rate: 48000, bits: 24, stereo: false, trim: false }.clean(), SoundOpts::default());
    }

    #[test]
    fn a_wav_that_fits_stays_as_it_is_and_one_that_doesnt_is_converted_with_its_loops() {
        let fine = write_wav(&tone(22050, 0.2, 1, 0.3), 22050, 16, None);
        let p = prepare("Fine.WAV", fine.clone(), SoundOpts::default()).unwrap();
        assert_eq!(p.bytes, fine);
        assert!(p.summary.is_none());
        assert_eq!(p.name, "Fine.wav");
        // 44.1 kHz stereo with a loop: mono 22 kHz, the loop at half the sample positions
        let marks = Marks { cues: vec![(1, 4410)], lengths: vec![(1, 8820)], loops: vec![(4410, 13230)] };
        let src = write_wav(&tone(44100, 0.5, 2, 0.5), 44100, 16, Some(&marks));
        let p = prepare("loop.wav", src, SoundOpts::default()).unwrap();
        let w = parse_wav(&p.bytes).unwrap();
        assert_eq!((w.channels, w.rate, w.bits), (1, 22050, 16));
        assert_eq!(w.marks, Marks { cues: vec![(1, 2205)], lengths: vec![(1, 4410)], loops: vec![(2205, 6615)] });
        assert_eq!(p.summary.as_deref(), Some("converted from 44.1 kHz stereo WAV to 22 kHz mono WAV, loop points kept"));
    }

    #[test]
    fn silence_trimmed_when_asked() {
        let mut c = vec![0f32; 2205];
        c.extend(tone(22050, 0.2, 1, 0.5).remove(0));
        c.extend(vec![0f32; 4410]);
        let src = write_wav(&[c], 22050, 16, None);
        let p = prepare("quiet.wav", src, SoundOpts { trim: true, ..Default::default() }).unwrap();
        let w = parse_wav(&p.bytes).unwrap();
        assert!(w.frames < 4410 + 600 && w.frames > 4410, "{} frames", w.frames);
        assert!(p.summary.unwrap().ends_with(", silence trimmed"));
    }

    #[test]
    fn the_resampler_keeps_a_tone_and_drops_what_the_new_rate_cant_hold() {
        // 440 Hz survives 44.1 → 22.05 kHz at about the same level
        let x = tone(44100, 0.2, 1, 0.5).remove(0);
        let y = resample(&x, 44100, 22050);
        assert_eq!(y.len(), x.len() / 2);
        let peak = y[200..y.len() - 200].iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!((0.45..0.55).contains(&peak), "{peak}");
        // 15 kHz is above the new Nyquist (11 kHz): filtered away, not folded back
        let hi: Vec<f32> = (0..8820).map(|i| 0.5 * (2.0 * std::f32::consts::PI * 15000.0 * i as f32 / 44100.0).sin()).collect();
        let z = resample(&hi, 44100, 22050);
        let peak = z[200..z.len() - 200].iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak < 0.02, "aliasing left {peak}");
    }

    #[test]
    fn bad_input_is_refused_with_a_reason() {
        assert!(prepare("x.mp3", b"not an mp3 at all, just text".to_vec(), SoundOpts::default()).unwrap_err().contains("couldn't be read as MP3"));
        assert!(prepare("x.wav", b"RIFX\0\0\0\0WAVE".to_vec(), SoundOpts::default()).unwrap_err().contains("isn't really a WAV"));
        assert!(prepare("x.ogg", vec![], SoundOpts::default()).unwrap_err().contains("empty"));
        assert!(prepare("x.flac", vec![0u8; MAX_BYTES + 1], SoundOpts::default()).unwrap_err().contains("bigger than"));
        // anything else passes untouched
        let p = prepare("box.mdl", b"IDST".to_vec(), SoundOpts::default()).unwrap();
        assert_eq!((p.name.as_str(), p.bytes.as_slice(), p.summary), ("box.mdl", &b"IDST"[..], None));
    }
}
