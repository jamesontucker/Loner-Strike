//! The game's voice packets (what the current Half-Life client sends), to sound.
//!
//! A packet: the speaker's SteamID (8 bytes), then records, then a CRC32 of everything before it:
//!   0x0B  u16 sample rate
//!   0x06  u16 length, then Opus chunks: u16 chunk length (0xFFFF: decoder reset, nothing more),
//!         u16 sequence number, the Opus frame (20 ms)
//!   0x00  u16 samples of silence
//! All numbers little-endian.

const MAX_FRAME: usize = 5760; // 120 ms at 48 kHz: the most one Opus frame can hold

fn u16le(b: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// The sample rate the packets say, 24000 when none does (what the client uses).
pub fn rate(packets: &[Vec<u8>]) -> u32 {
    for p in packets {
        if p.len() >= 12 && p[8] == 0x0B {
            if let Some(r) = u16le(p, 9) {
                if matches!(r, 8000 | 12000 | 16000 | 24000 | 48000) {
                    return r as u32;
                }
            }
        }
    }
    24000
}

/// One talk's packets as mono 16-bit sound at `rate(packets)`. Packets that fail their checksum
/// or don't parse are skipped.
pub fn decode(packets: &[Vec<u8>]) -> Vec<i16> {
    let sample_rate = rate(packets);
    let mut dec = match opus::Decoder::new(sample_rate, opus::Channels::Mono) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut frame = vec![0i16; MAX_FRAME];
    for p in packets {
        if p.len() < 12 {
            continue;
        }
        let end = p.len() - 4;
        let want = u32::from_le_bytes([p[end], p[end + 1], p[end + 2], p[end + 3]]);
        if crc32(&p[..end]) != want {
            continue;
        }
        let b = &p[..end];
        let mut i = 8;
        while i < b.len() {
            let kind = b[i];
            i += 1;
            match kind {
                0x0B => i += 2,
                0x00 => {
                    let Some(n) = u16le(b, i) else { break };
                    i += 2;
                    out.extend(std::iter::repeat(0i16).take(n.min(sample_rate as usize)));
                }
                0x06 => {
                    let Some(len) = u16le(b, i) else { break };
                    i += 2;
                    let stop = (i + len).min(b.len());
                    while i + 2 <= stop {
                        let Some(cl) = u16le(b, i) else { break };
                        i += 2;
                        if cl == 0xFFFF {
                            let _ = dec.reset_state();
                            continue;
                        }
                        i += 2; // (the sequence number)
                        if i + cl > stop {
                            break;
                        }
                        if let Ok(n) = dec.decode(&b[i..i + cl], &mut frame, false) {
                            out.extend_from_slice(&frame[..n]);
                        }
                        i += cl;
                    }
                    i = stop;
                }
                _ => break,
            }
        }
    }
    out
}

/// Sound at `from` Hz to 16 kHz floats in -1..1, what Whisper takes: a little smoothing first
/// when going down (so the higher sounds don't fold back in), then straight-line in between.
pub fn to_16k(pcm: &[i16], from: u32) -> Vec<f32> {
    let x: Vec<f32> = pcm.iter().map(|&s| s as f32 / 32768.0).collect();
    if from == 16000 || x.is_empty() {
        return x;
    }
    let smooth: Vec<f32> = if from > 16000 {
        (0..x.len())
            .map(|i| {
                let a = x[i.saturating_sub(1)];
                let c = x[(i + 1).min(x.len() - 1)];
                0.25 * a + 0.5 * x[i] + 0.25 * c
            })
            .collect()
    } else {
        x
    };
    let ratio = from as f64 / 16000.0;
    let n = (smooth.len() as f64 / ratio) as usize;
    (0..n)
        .map(|k| {
            let pos = k as f64 * ratio;
            let i = pos as usize;
            let f = (pos - i as f64) as f32;
            let a = smooth[i.min(smooth.len() - 1)];
            let b = smooth[(i + 1).min(smooth.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_zlib() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn resample_length() {
        let pcm = vec![0i16; 24000];
        assert_eq!(to_16k(&pcm, 24000).len(), 16000);
    }
}
