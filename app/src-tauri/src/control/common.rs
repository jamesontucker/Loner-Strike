//! Small checks and filters shared word for word by the app and `ls-control` (the shared
//! control service on a Linux server, app/server-agent), which compiles this file in with
//! `#[path]`. Plain std only: nothing from the app's crate.

/// A SteamID as the game writes it: STEAM_X:Y:Z (X 0 to 5, Y 0 or 1, Z a number).
pub fn valid_steamid(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("STEAM_") else { return false };
    let mut parts = rest.split(':');
    let (Some(x), Some(y), Some(z), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return false };
    let digits = |p: &str, max: usize| !p.is_empty() && p.len() <= max && p.bytes().all(|c| c.is_ascii_digit());
    digits(x, 1) && x.as_bytes()[0] <= b'5' && (y == "0" || y == "1") && digits(z, 10) && z.parse::<u32>().is_ok() && !(z.len() > 1 && z.starts_with('0'))
}

/// What a Moderator is told when the player is the owner, an Admin, a Moderator or a helper.
pub const OUTRANKED: &str = "Moderators can't ban, mute, kick, slap or slay the server's owner, its Admins or Moderators, or anyone helping run it.";

/// The SteamID of the player with this userid, from the game's `status` answer
/// (`#  2 "Name" 7 STEAM_0:1:123 0 05:00 30 0 1.2.3.4:27005`). None: not in the game.
pub fn steamid_for_uid(status: &str, uid: u32) -> Option<String> {
    status.lines().filter(|l| l.trim_start().starts_with('#')).find_map(|l| {
        let (_, rest) = l.rsplit_once('"')?;
        let mut w = rest.split_whitespace();
        (w.next()?.parse::<u32>().ok()? == uid).then(|| w.next().map(String::from)).flatten()
    })
}

/// Whether two SteamIDs are the same account. The game ignores the first number (the
/// "universe": STEAM_0 and STEAM_1 are one account), so only Y:Z counts.
pub fn same_steamid(a: &str, b: &str) -> bool {
    let tail = |s: &str| s.trim().to_ascii_uppercase().strip_prefix("STEAM_").and_then(|r| r.split_once(':')).map(|(_, yz)| yz.to_string());
    match (tail(a), tail(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a.trim().eq_ignore_ascii_case(b.trim()),
    }
}

/// A command argument the panel may send: plain words only (the UI checks the same rule).
/// ('$' for maps like $2000$: the game's console doesn't expand it, and the commands sent to a
/// server on another machine quote every argument)
pub fn safe_arg(a: &str) -> bool {
    !a.is_empty() && a.len() <= 64 && a.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-$".contains(&c))
}

/// gm_announce's free text: only characters that can't end or extend the console command.
pub fn announce_text(args: &[String]) -> String {
    let text: String = args.join(" ").chars().filter(|c| c.is_ascii_alphanumeric() || " _.,!?'()+-:#@&%/".contains(*c)).take(120).collect();
    text.trim().to_string()
}

/// A kick reason the player sees: letters, digits and plain punctuation only, so it can't
/// end or extend the console command.
pub fn kick_reason(s: &str) -> String {
    let kept: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || " _.,!?'()+-:#@&%/".contains(*c)).take(64).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The engine's own kick by userid, with the reason shown to them when there is one.
pub fn kick_line(uid: u32, reason: &str) -> String {
    let reason = kick_reason(reason);
    if reason.is_empty() {
        format!("kick #{uid}")
    } else {
        format!("kick #{uid} \"{reason}\"")
    }
}

/// The console without the app's own checks every few seconds ("Rcon from <from>", the
/// address the app knocks on, and the gm_status it sent), which would bury chat and
/// everything else; nor what the console's suggestions ask for (the command lists, and a
/// setting's value: one word on its own).
pub fn without_polls(lines: Vec<String>, from: &str, is_setting: &dyn Fn(&str) -> bool) -> Vec<String> {
    let ours = format!("Rcon from {from}:");
    let mut after_ours = false;
    lines
        .into_iter()
        .filter(|l| {
            let t = l.trim();
            let was_ours = std::mem::replace(&mut after_ours, t.starts_with(&ours) || t.starts_with("Rcon from 127.0.0.1:"));
            if after_ours {
                return false;
            }
            if !t.starts_with("rcon ") {
                return true;
            }
            if t.ends_with(" gm_status") || t.ends_with(" gm_status_full") {
                return false;
            }
            // the voice add-on telling the server where to send voice, every 30 s
            if t.splitn(2, "\" ").nth(1).is_some_and(|c| c.trim().starts_with("sv_ls_voice_feed ")) {
                return false;
            }
            // rcon <challenge> "<password>" <command>
            let cmd = t.splitn(2, "\" ").nth(1).unwrap_or("").trim();
            let lists = matches!(cmd, "cmdlist" | "cvarlist" | "amxx cmds" | "amxx cvars") || cmd.strip_prefix("amx_help ").is_some_and(|n| n.parse::<u32>().is_ok());
            // a setting's value looked up for the suggestions (a known setting on its own);
            // a one-word command typed in the console (status) stays
            !(was_ours && (lists || (!cmd.is_empty() && !cmd.contains(' ') && is_setting(cmd))))
        })
        .collect()
}

/// The server logs every RCON command with its password ("rcon 1234 "secret" status"),
/// and rcon_password / sv_password changes too: blank them out before showing the console.
pub fn hide_passwords(line: &str) -> String {
    // a setting as cvarlist lists it ("rcon_password                : secret, sv"), or
    // as the game answers its name ("rcon_password" is "secret"): a key's value blanked
    if let Some((name, _)) = line.split_once(" : ") {
        let name = name.trim();
        if !name.contains(' ') && super::rules::secret_setting(name) {
            return format!("{} : ••••", line.split_once(" : ").map_or("", |p| p.0));
        }
    }
    if let Some(rest) = line.trim_start().strip_prefix('"') {
        if let Some((name, after)) = rest.split_once('"') {
            if after.trim_start().starts_with("is ") && super::rules::secret_setting(name) {
                return format!("\"{name}\" is \"••••\"");
            }
        }
    }
    let lower = line.to_lowercase();
    for key in ["rcon ", "rcon_password", "sv_password", "password "] {
        if let Some(i) = lower.find(key) {
            if let Some(q) = line[i..].find('"').map(|q| i + q) {
                if let Some(end) = line[q + 1..].find('"').map(|e| q + 1 + e) {
                    return format!("{}\"••••\"{}", &line[..q], hide_passwords(&line[end + 1..]));
                }
            }
        }
    }
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_account_any_universe() {
        assert!(same_steamid("STEAM_0:1:200", "STEAM_1:1:200") && same_steamid("steam_0:0:5", "STEAM_0:0:5"));
        assert!(!same_steamid("STEAM_0:1:200", "STEAM_0:0:200") && !same_steamid("STEAM_0:1:200", "STEAM_0:1:2000"));
    }

    #[test]
    fn userids_to_steamids() {
        let st = "# userid name uniqueid connected ping loss state rate adr\n#  1 \"Pat \"the\" one\" 3 STEAM_0:1:42 0 05:00 30 0 1.2.3.4:27005\n#  2 \"Bot\" 4 BOT 0 05:00 0 0\n";
        assert_eq!(steamid_for_uid(st, 3).as_deref(), Some("STEAM_0:1:42"));
        assert_eq!(steamid_for_uid(st, 4).as_deref(), Some("BOT"));
        assert_eq!(steamid_for_uid(st, 5), None);
    }

    #[test]
    fn setting_lists_hide_keys() {
        for (line, secret) in [("rcon_password                :       Abc123secret, sv", "Abc123secret"), ("sv_password     : hunter2, sv", "hunter2"), ("gm_webhook_url  : https://x", "https://x")] {
            let out = hide_passwords(line);
            assert!(!out.contains(secret) && out.ends_with(": ••••") && out.starts_with(line.split(' ').next().unwrap()), "{out}");
        }
        assert_eq!(hide_passwords(r#""rcon_password" is "Abc123secret""#), r#""rcon_password" is "••••""#);
        assert_eq!(hide_passwords("mp_roundtime    :      2.5, sv"), "mp_roundtime    :      2.5, sv");
        assert_eq!(hide_passwords(r#""mp_timelimit" is "20""#), r#""mp_timelimit" is "20""#);
    }

    #[test]
    fn steamids() {
        assert!(valid_steamid("STEAM_0:1:12345") && valid_steamid("STEAM_5:0:0"));
        for bad in ["", "bob", "STEAM_0:1:1; rcon_password x", "STEAM_6:0:1", "STEAM_0:2:1", "STEAM_0:1:01", "STEAM_0:1:", "steam_0:1:1", "STEAM_0:1:99999999999"] {
            assert!(!valid_steamid(bad), "{bad}");
        }
    }

    #[test]
    fn words_and_reasons() {
        assert!(safe_arg("de_dust2") && !safe_arg("a;b") && !safe_arg("") && !safe_arg(&"a".repeat(65)));
        assert_eq!(kick_line(5, "spam; quit"), "kick #5 \"spam quit\"");
        assert_eq!(kick_line(5, ""), "kick #5");
        assert_eq!(announce_text(&["hi\";".into(), "all".into()]), "hi all");
    }
}

