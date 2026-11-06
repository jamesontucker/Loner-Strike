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
    /// A server on another machine: commands go through the SSH session to a helper
    /// there (lsrcon) that talks to the game locally, so the password never crosses
    /// the internet. (machine, the password file there)
    via: Option<(crate::remote::RemoteInfo, String)>,
}

/// GoldSrc text is Latin-1: every byte is one character.
pub fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

impl Rcon {
    pub fn new(password: String, port: u16) -> Self {
        Self::at(password, "127.0.0.1", port)
    }

    /// A game on this computer listening on one address of its own (a server's listen address).
    pub fn at(password: String, host: &str, port: u16) -> Self {
        Rcon { password, addr: format!("{host}:{port}"), lock: Mutex::new(()), via: None }
    }

    /// A server on another machine, reached through SSH (`pw_file`: its password file there).
    pub fn over_ssh(r: crate::remote::RemoteInfo, pw_file: String, password: String, port: u16) -> Self {
        Rcon { password, addr: format!("127.0.0.1:{port}"), lock: Mutex::new(()), via: Some((r, pw_file)) }
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn send(&self, command: &str) -> Result<String, RconError> {
        let _guard = self.lock.lock().unwrap();
        if let Some((r, pw_file)) = &self.via {
            return match crate::remote::exec_quick(r, &self.lsrcon(pw_file, command)) {
                Ok((0, out)) if out.contains("Bad rcon_password") => Err(RconError::BadPassword),
                Ok((0, out)) => Ok(out),
                _ => Err(RconError::Offline),
            };
        }
        self.send_udp(command)
    }

    /// Sends a command, then (on another machine, in the same SSH command) reads a file
    /// there it has just written: (reply, the file). On this computer the file is None.
    pub fn send_then_cat(&self, command: &str, file: &str, mark: &str) -> Result<(String, Option<Vec<u8>>), RconError> {
        let Some((r, pw_file)) = &self.via else { return self.send(command).map(|o| (o, None)) };
        let _guard = self.lock.lock().unwrap();
        let printf_mark = mark.replace('\n', "\\n");
        let cmd = format!(
            "{{ {} && {{ printf {}; cat {} 2>/dev/null; true; }}; }}",
            self.lsrcon(pw_file, command),
            crate::remote::shell_quote(&printf_mark),
            crate::remote::shell_quote(file)
        );
        match crate::remote::exec_quick(r, &cmd) {
            Ok((0, out)) => {
                let (reply, rest) = out.split_once(mark).unwrap_or((&out, ""));
                if reply.contains("Bad rcon_password") {
                    return Err(RconError::BadPassword);
                }
                // exec's text is Latin-1, one character per byte: back to the bytes
                Ok((reply.to_string(), Some(rest.chars().map(|c| c as u8).collect())))
            }
            _ => Err(RconError::Offline),
        }
    }

    fn lsrcon(&self, pw_file: &str, command: &str) -> String {
        let port = self.addr.rsplit(':').next().unwrap_or("27015");
        format!("lsrcon {port} {} {}", crate::remote::shell_quote(pw_file), crate::remote::shell_quote(command))
    }

    fn send_udp(&self, command: &str) -> Result<String, RconError> {
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

        // The first packet may take a while (still 1.5 s); after it the rest of the answer
        // follows right away, so a short quiet spell ends it. Only a split answer still
        // missing parts is waited for longer. (The lock is held all the while.)
        let mut got = Packets::default();
        while let Ok(n) = sock.recv(&mut buf) {
            got.add(&buf[..n]);
            let wait = if got.missing_parts() { MISSING_PART_WAIT } else { QUIET_WAIT };
            sock.set_read_timeout(Some(wait)).ok();
        }
        let text = got.text();
        if text.contains("Bad rcon_password") {
            return Err(RconError::BadPassword);
        }
        Ok(text)
    }
}

/// After the answer's first packet: this long without another ends it
const QUIET_WAIT: Duration = Duration::from_millis(40);
/// ...unless a split answer is still missing parts
const MISSING_PART_WAIT: Duration = Duration::from_millis(400);

/// An answer's packets: whole ones, and long answers split into numbered parts
/// (0xFE header; byte 8 is the part's number << 4 | how many parts there are).
#[derive(Default)]
struct Packets {
    singles: Vec<Vec<u8>>,
    /// (answer id, how many parts, the parts so far)
    splits: Vec<(Vec<u8>, u8, Vec<(u8, Vec<u8>)>)>,
}

impl Packets {
    fn add(&mut self, d: &[u8]) {
        if d.len() > 9 && d[..4] == [0xfe, 0xff, 0xff, 0xff] {
            let id = d[4..8].to_vec();
            let (num, total) = (d[8] >> 4, d[8] & 0x0f);
            let part = (num, d[9..].to_vec());
            match self.splits.iter_mut().find(|(k, _, _)| *k == id) {
                Some((_, _, v)) => {
                    if !v.iter().any(|p| p.0 == num) {
                        v.push(part)
                    }
                }
                None => self.splits.push((id, total, vec![part])),
            }
        } else {
            self.singles.push(d.to_vec());
        }
    }

    /// A split answer that hasn't all its parts yet.
    fn missing_parts(&self) -> bool {
        self.splits.iter().any(|(_, total, parts)| parts.len() < *total as usize)
    }

    fn text(self) -> String {
        let mut all = self.singles;
        for (_, _, mut parts) in self.splits {
            parts.sort_by_key(|p| p.0);
            all.push(parts.into_iter().flat_map(|p| p.1).collect());
        }
        all.iter().map(|d| latin1(if d.starts_with(PREFIX) && d.len() > 5 { &d[5..] } else { d })).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(id: u8, num: u8, total: u8, body: &[u8]) -> Vec<u8> {
        let mut d = vec![0xfe, 0xff, 0xff, 0xff, id, 0, 0, 0, num << 4 | total];
        d.extend_from_slice(body);
        d
    }

    #[test]
    fn split_answers() {
        let mut p = Packets::default();
        assert!(!p.missing_parts());
        p.add(&part(1, 1, 3, b"lo "));
        assert!(p.missing_parts());
        p.add(&part(1, 0, 3, b"\xff\xff\xff\xfflhel"));
        p.add(&part(1, 1, 3, b"lo ")); // sent twice
        assert!(p.missing_parts());
        p.add(&part(1, 2, 3, b"world"));
        assert!(!p.missing_parts());
        assert_eq!(p.text(), "hello world");

        let mut p = Packets::default();
        p.add(b"\xff\xff\xff\xfflGM status written\n");
        assert!(!p.missing_parts());
        assert_eq!(p.text(), "GM status written\n");
    }
}
