//! GoldSrc RCON over UDP: ask for a challenge, then send the command with it.

use std::net::UdpSocket;
use std::sync::Mutex;
use std::time::Duration;

const PREFIX: &[u8] = b"\xff\xff\xff\xff";

#[derive(Debug)]
pub enum RconError {
    /// The game isn't running, or no match is loaded.
    Offline,
    /// The game rejected the password (it changes when a map loads for the first time).
    BadPassword,
}

pub struct Rcon {
    password: String,
    addr: String,
    lock: Mutex<()>,
}

/// GoldSrc text is Latin-1: every byte is one character.
pub fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

impl Rcon {
    pub fn new(password: String, port: u16) -> Self {
        Rcon { password, addr: format!("127.0.0.1:{port}"), lock: Mutex::new(()) }
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn send(&self, command: &str) -> Result<String, RconError> {
        let _guard = self.lock.lock().unwrap();
        let sock = UdpSocket::bind("127.0.0.1:0").map_err(|_| RconError::Offline)?;
        sock.set_read_timeout(Some(Duration::from_millis(1500))).ok();
        sock.connect(&self.addr).map_err(|_| RconError::Offline)?;

        let mut msg = PREFIX.to_vec();
        msg.extend_from_slice(b"challenge rcon\n");
        sock.send(&msg).map_err(|_| RconError::Offline)?;
        let mut buf = vec![0u8; 65535];
        let n = sock.recv(&mut buf).map_err(|_| RconError::Offline)?;
        let reply = latin1(&buf[4.min(n)..n]);
        let parts: Vec<&str> = reply.split_whitespace().collect();
        if parts.len() < 3 || parts[0] != "challenge" {
            return Err(RconError::Offline);
        }

        let mut msg = PREFIX.to_vec();
        msg.extend(format!("rcon {} \"{}\" {}\n", parts[2], self.password, command).chars().map(|c| c as u8));
        sock.send(&msg).map_err(|_| RconError::Offline)?;

        // Long answers come back split into numbered packets
        sock.set_read_timeout(Some(Duration::from_millis(400))).ok();
        let mut singles: Vec<Vec<u8>> = Vec::new();
        let mut splits: Vec<(Vec<u8>, Vec<(u8, Vec<u8>)>)> = Vec::new();
        while let Ok(n) = sock.recv(&mut buf) {
            let d = &buf[..n];
            if d.len() > 9 && d[..4] == [0xfe, 0xff, 0xff, 0xff] {
                let id = d[4..8].to_vec();
                let part = (d[8] >> 4, d[9..].to_vec());
                match splits.iter_mut().find(|(k, _)| *k == id) {
                    Some((_, v)) => v.push(part),
                    None => splits.push((id, vec![part])),
                }
            } else {
                singles.push(d.to_vec());
            }
        }
        for (_, mut parts) in splits {
            parts.sort_by_key(|p| p.0);
            singles.push(parts.into_iter().flat_map(|p| p.1).collect());
        }
        let text: String = singles
            .iter()
            .map(|d| latin1(if d.starts_with(PREFIX) && d.len() > 5 { &d[5..] } else { d }))
            .collect();
        if text.contains("Bad rcon_password") {
            return Err(RconError::BadPassword);
        }
        Ok(text)
    }
}
