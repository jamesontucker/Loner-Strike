//! The server's remote console (GoldSrc rcon over UDP): a challenge, then the command with the
//! password. Only ever to 127.0.0.1: ls-voice runs on the same machine as its server.

use std::net::UdpSocket;
use std::time::Duration;

pub struct Rcon {
    port: u16,
    password: String,
}

const HEAD: &[u8] = b"\xff\xff\xff\xff";

impl Rcon {
    pub fn new(port: u16, password: String) -> Rcon {
        Rcon { port, password }
    }

    /// Runs a command; what the server printed back (empty when it didn't answer).
    pub fn cmd(&self, command: &str) -> String {
        for _ in 0..3 {
            if let Some(out) = self.try_cmd(command) {
                return out;
            }
        }
        String::new()
    }

    fn try_cmd(&self, command: &str) -> Option<String> {
        let s = UdpSocket::bind("127.0.0.1:0").ok()?;
        s.set_read_timeout(Some(Duration::from_millis(1500))).ok()?;
        let to = ("127.0.0.1", self.port);
        s.send_to(&[HEAD, b"challenge rcon\n"].concat(), to).ok()?;
        let mut buf = [0u8; 65536];
        let n = s.recv(&mut buf).ok()?;
        // "\xff\xff\xff\xffchallenge rcon <number>\n"
        let text = String::from_utf8_lossy(&buf[4.min(n)..n]).to_string();
        let challenge = text.split_whitespace().nth(2)?.to_string();
        let line = format!("rcon {} \"{}\" {}\n", challenge, self.password, command);
        s.send_to(&[HEAD, line.as_bytes()].concat(), to).ok()?;
        // (the answer can come in several packets; a short wait after the first for the rest)
        s.set_read_timeout(Some(Duration::from_millis(250))).ok()?;
        let mut out = String::new();
        while let Ok(n) = s.recv(&mut buf) {
            // "\xff\xff\xff\xffl" then the text
            if n > 5 {
                out.push_str(&String::from_utf8_lossy(&buf[5..n]));
            }
        }
        Some(out)
    }
}

/// From "status": the bots' names, and the map.
pub fn status(text: &str) -> (Vec<String>, String) {
    let mut bots = Vec::new();
    let mut map = String::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("map") {
            if let Some(v) = rest.split(':').nth(1) {
                map = v.split_whitespace().next().unwrap_or("").to_string();
            }
            continue;
        }
        // #  2 "Name" 3 BOT ...
        if !l.starts_with('#') || !l.contains(" BOT") {
            continue;
        }
        if let (Some(a), Some(b)) = (l.find('"'), l.rfind('"')) {
            if b > a {
                bots.push(l[a + 1..b].to_string());
            }
        }
    }
    (bots, map)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_status() {
        let t = "hostname:  Tester\nmap     :  de_dust2 at: 0 x, 0 y, 0 z\n# 1 \"nanoshrine\" 1 STEAM_0:0:1 0 02:14 33 0 127.0.0.1:27005\n# 2 \"Derek S.\" 20 BOT   0 03:21    0    0\n#10 \"::CoD:: Gill\" 21 BOT 0 03:21 0 0\n";
        let (bots, map) = super::status(t);
        assert_eq!(map, "de_dust2");
        assert_eq!(bots, vec!["Derek S.", "::CoD:: Gill"]);
    }
}
