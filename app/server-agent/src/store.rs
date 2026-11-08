//! People, invites and the activity log for one server, in ~/loner-strike/control/<id>/.
//! The same rules as the owner's app (control/mod.rs): invites work once, for one person,
//! for a day; only the sha256 of an invite's secret and of a person's key is kept.
//! The service and the owner's commands over SSH both change store.json: under a file
//! lock, read fresh every time, written whole and renamed into place.

use crate::{Config, Role};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Unused invites a server can have at once
const MAX_INVITES: usize = 20;
/// How long an invite works
const INVITE_HOURS: u64 = 24;
/// The invite's prefix: what it is, and the format's version (the app's INVITE_PREFIX)
const INVITE_PREFIX: &str = "LS1.";
/// People a server can have
const MAX_MEMBERS: usize = 100;
/// The activity log is cut back to its newest half past this many bytes
const LOG_TRIM_AT: u64 = 2 * 1024 * 1024;
const LOG_KEEP: usize = 3000;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub role: Role,
    /// the server they were given
    pub server: String,
    /// sha256 of their key (the key itself only ever goes to their app)
    pub key: String,
    /// their SteamID (optional)
    #[serde(default)]
    pub steam: String,
    pub added: String,
    /// "owner", or the member id of the Admin who invited them
    #[serde(default)]
    pub invited_by: String,
    #[serde(default)]
    pub seen: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Invite {
    pub id: String,
    /// sha256 of the invite's secret
    pub code: String,
    pub role: Role,
    pub server: String,
    #[serde(default)]
    pub steam: String,
    /// unix seconds
    pub expires: u64,
    pub by: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Store {
    #[serde(default)]
    pub members: Vec<Member>,
    #[serde(default)]
    pub invites: Vec<Invite>,
}

/// What an invite carries (base64 JSON after LS1.), as the app's control::InviteText: here
/// one https:// address and no certificate fingerprint (Caddy or Cloudflare has a real one).
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct InviteText {
    pub a: Vec<String>,
    pub p: u16,
    pub f: String,
    pub c: String,
    pub s: String,
    pub r: Role,
}

pub fn encode_invite(t: &InviteText) -> String {
    format!("{INVITE_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(t).unwrap_or_default()))
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// "20261107-142233" (UTC), like the app's sys::stamp
pub fn stamp() -> String {
    let secs = now() as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    // no randomness: nothing is made (a predictable key would be worse than none)
    getrandom::getrandom(&mut b).expect("the system's random numbers");
    hex(&b)
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn sha(s: &str) -> String {
    hex(&Sha256::digest(s.as_bytes()))
}

/// Equal, in a time that doesn't depend on where they differ.
pub fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|c| c.is_ascii_hexdigit())
}

pub fn member_view(m: &Member) -> Value {
    json!({"id": m.id, "name": m.name, "role": m.role, "steam": m.steam, "added": m.added, "seen": m.seen, "invited_by": m.invited_by})
}

/// A name someone gave themselves: one line, at most 32 characters.
fn clean_name(name: &str) -> String {
    let n: String = name.trim().chars().filter(|c| !c.is_control()).take(32).collect();
    if n.trim().is_empty() {
        "Someone".into()
    } else {
        n.trim().to_string()
    }
}

/// One server's files.
pub struct Files {
    dir: PathBuf,
    server: String,
}

/// One change at a time within this process (the file lock covers the others)
static LOCAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl Files {
    pub fn new(p: &crate::Paths) -> Files {
        Files { dir: p.control_dir(), server: p.id.clone() }
    }

    fn store_path(&self) -> PathBuf {
        self.dir.join("store.json")
    }

    pub fn log_path(&self) -> PathBuf {
        self.dir.join("log.jsonl")
    }

    /// What's saved: empty when there's nothing yet, an error when it's there but can't be
    /// read (so nothing is ever saved over it).
    pub fn read(&self) -> Result<Store, String> {
        match std::fs::read_to_string(self.store_path()) {
            Ok(t) => serde_json::from_str(&t).map_err(|_| "The shared control file on the server is damaged.".to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
            Err(e) => Err(format!("Couldn't read the shared control file on the server ({e}).")),
        }
    }

    /// Changes the store under its lock and saves it (only this user can read it).
    pub fn edit<T>(&self, f: impl FnOnce(&mut Store) -> Result<T, String>) -> Result<T, String> {
        let _local = LOCAL.lock().unwrap_or_else(|e| e.into_inner());
        make_private_dir(&self.dir)?;
        let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(self.dir.join(".lock")).map_err(|e| format!("Couldn't lock the shared control file ({e})."))?;
        lock.lock().map_err(|e| format!("Couldn't lock the shared control file ({e})."))?;
        let mut s = self.read()?;
        let out = f(&mut s)?;
        write_private(&self.store_path(), serde_json::to_string_pretty(&s).unwrap_or_default().as_bytes())?;
        drop(lock);
        Ok(out)
    }

    /// A new invite (one person, one day). `by`: "owner", or the inviting Admin's id.
    pub fn invite(&self, config: &Config, role: Role, steam: &str, by: &str) -> Result<(String, String), String> {
        let steam = steam.trim().to_uppercase();
        if !steam.is_empty() && !crate::common::valid_steamid(&steam) {
            return Err("That isn't a SteamID. It looks like STEAM_0:1:12345.".into());
        }
        if config.url.is_empty() {
            return Err("Helpers reach this server at its website status address, and it has none yet. Set up Website status for it first.".into());
        }
        let secret = random_hex(16);
        let id = random_hex(4);
        let server = self.server.clone();
        self.edit(|s| {
            s.invites.retain(|i| i.expires > now());
            if s.invites.iter().filter(|i| i.server == server).count() >= MAX_INVITES {
                return Err(format!("There are already {MAX_INVITES} unused invites for this server. Wait for them to be used or run out (a day)."));
            }
            s.invites.push(Invite { id: id.clone(), code: sha(&secret), role, server: server.clone(), steam: steam.clone(), expires: now() + INVITE_HOURS * 3600, by: by.into() });
            Ok(())
        })?;
        Ok((id, encode_invite(&InviteText { a: vec![config.url.clone()], p: 443, f: String::new(), c: secret, s: config.name(), r: role })))
    }

    /// Turns an invite's secret into a person: their key comes back (once).
    pub fn redeem(&self, secret: &str, name: &str) -> Result<(Member, String), String> {
        const GONE: &str = "That invite doesn't work any more. Ask for a new one.";
        let secret = secret.trim();
        if !is_hex(secret, 32) {
            return Err(GONE.into());
        }
        let name = clean_name(name);
        let key = random_hex(32);
        let code = sha(secret);
        let server = self.server.clone();
        self.edit(|s| {
            // every invite is compared, matched or not (no early way out to time)
            let mut at = None;
            for (i, inv) in s.invites.iter().enumerate() {
                if ct_eq(&inv.code, &code) && at.is_none() {
                    at = Some(i);
                }
            }
            let i = at.filter(|&i| s.invites[i].expires > now() && s.invites[i].server == server).ok_or(GONE)?;
            if s.members.len() >= MAX_MEMBERS {
                return Err("This server already has as many helpers as it can.".into());
            }
            let inv = s.invites.remove(i);
            let m = Member { id: random_hex(6), name, role: inv.role, server: inv.server, key: sha(&key), steam: inv.steam, added: stamp(), invited_by: inv.by, seen: Some(now()) };
            s.members.push(m.clone());
            Ok(m)
        })
        .map(|m| (m, key))
    }

    /// The person with this key (compared with everyone's, in constant time).
    pub fn member_for(&self, key: &str) -> Result<Option<Member>, String> {
        if !is_hex(key, 64) {
            return Ok(None);
        }
        let h = sha(key);
        let mut found = None;
        for m in self.read()?.members {
            if ct_eq(&m.key, &h) && found.is_none() {
                found = Some(m);
            }
        }
        Ok(found)
    }

    /// Notes that someone was here (saved at most once a minute).
    pub fn seen(&self, id: &str) {
        let due = self.read().map(|s| s.members.iter().any(|m| m.id == id && m.seen.map_or(true, |t| now() > t + 60))).unwrap_or(false);
        if due {
            let _ = self.edit(|s| {
                if let Some(m) = s.members.iter_mut().find(|m| m.id == id) {
                    m.seen = Some(now());
                }
                Ok(())
            });
        }
    }

    pub fn set_role(&self, mid: &str, role: Role) -> Result<Member, String> {
        self.edit(|s| {
            let m = s.members.iter_mut().find(|m| m.id == mid).ok_or("They don't have access any more.")?;
            m.role = role;
            Ok(m.clone())
        })
    }

    /// Takes someone's access away, with the invites they made and the people who came in
    /// through them (an Admin's Moderators). Who went comes back.
    pub fn remove(&self, mid: &str) -> Result<Vec<Member>, String> {
        self.edit(|s| {
            let i = s.members.iter().position(|m| m.id == mid).ok_or("They don't have access any more.")?;
            s.invites.retain(|inv| inv.by != mid);
            let mut gone = vec![s.members.remove(i)];
            let (theirs, rest): (Vec<Member>, Vec<Member>) = std::mem::take(&mut s.members).into_iter().partition(|m| m.invited_by == mid);
            s.members = rest;
            s.invites.retain(|inv| !theirs.iter().any(|t| t.id == inv.by));
            gone.extend(theirs);
            Ok(gone)
        })
    }

    pub fn cancel(&self, iid: &str) -> Result<(), String> {
        self.edit(|s| {
            let n = s.invites.len();
            s.invites.retain(|i| i.id != iid);
            if s.invites.len() == n {
                return Err("That invite was already used, cancelled or ran out.".into());
            }
            Ok(())
        })
    }

    /// The owner's view (over SSH): people, open invites and the activity, never a key.
    pub fn view(&self, server: &str, log: usize) -> Result<Value, String> {
        let s = self.read()?;
        let by = |b: &str| if b == "owner" { "you".to_string() } else { s.members.iter().find(|m| m.id == b).map(|m| m.name.clone()).unwrap_or_else(|| "an Admin".into()) };
        let members: Vec<Value> = s.members.iter().filter(|m| m.server == server).map(member_view).collect();
        let invites: Vec<Value> = s.invites.iter().filter(|i| i.server == server && i.expires > now()).map(|i| json!({"id": i.id, "role": i.role, "steam": i.steam, "expires": i.expires, "by": by(&i.by)})).collect();
        Ok(json!({"members": members, "invites": invites, "activity": self.activity(log), "version": crate::VERSION}))
    }

    // ---- activity -----------------------------------------------------------------------

    /// A change someone made goes in the log (bodies without anything secret-looking).
    pub fn log_change(&self, m: &Member, path: &str, body: Option<&Value>) {
        let body = body.map(|b| short(&scrub(b))).unwrap_or(Value::Null);
        let line = json!({"id": random_hex(6), "at": now(), "who": m.name, "member": m.id, "role": m.role, "path": path, "body": body, "undo": null});
        let _one = LOCAL.lock().unwrap_or_else(|e| e.into_inner());
        if make_private_dir(&self.dir).is_err() {
            return;
        }
        let file = self.log_path();
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        if let Ok(mut f) = opts.open(&file) {
            let _ = writeln!(f, "{line}");
        }
        // cut back to its newest half once it gets big
        if std::fs::metadata(&file).map(|m| m.len() > LOG_TRIM_AT).unwrap_or(false) {
            if let Ok(t) = std::fs::read_to_string(&file) {
                let mut lines: Vec<&str> = t.lines().collect();
                let mut size: usize = lines.iter().map(|l| l.len() + 1).sum();
                while size > (LOG_TRIM_AT / 2) as usize && lines.len() > 1 {
                    size -= lines.remove(0).len() + 1;
                }
                let keep = lines[lines.len().saturating_sub(LOG_KEEP)..].join("\n");
                let _ = write_private(&file, format!("{keep}\n").as_bytes());
            }
        }
    }

    /// The log, newest first, as the owner's page shows it (no undo from here yet).
    pub fn activity(&self, limit: usize) -> Vec<Value> {
        let Ok(t) = std::fs::read_to_string(self.log_path()) else { return vec![] };
        let mut v: Vec<Value> = t.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect();
        let skip = v.len().saturating_sub(limit);
        v.drain(..skip);
        v.reverse();
        for e in v.iter_mut() {
            if let Some(o) = e.as_object_mut() {
                o.remove("undo");
                o.insert("can_undo".into(), json!(false));
                o.insert("undone".into(), json!(false));
            }
        }
        v
    }
}

/// A copy of a body without values whose names look secret.
pub fn scrub(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), if crate::rules::secret_setting(k) { json!("…") } else { scrub(v) })).collect()),
        Value::Array(a) => Value::Array(a.iter().map(scrub).collect()),
        other => other.clone(),
    }
}

/// What a change carried, kept short for the log.
fn short(v: &Value) -> Value {
    match v {
        Value::String(s) if s.chars().count() > 200 => json!(format!("{}…", s.chars().take(200).collect::<String>())),
        Value::Object(o) if o.len() > 40 => json!(format!("{} settings", o.len())),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), short(v))).collect()),
        Value::Array(a) if a.len() > 20 => json!(format!("{} items", a.len())),
        Value::Array(a) => Value::Array(a.iter().map(short).collect()),
        other => other.clone(),
    }
}

/// A folder only this user can open.
pub fn make_private_dir(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't make {} ({e}).", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Writes a file only this user can read, whole: to a temporary file renamed over it.
pub fn write_private(path: &std::path::Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let tmp = path.with_extension(format!("tmp-{}", random_hex(4)));
    let mut opts = std::fs::OpenOptions::new();
    opts.create_new(true).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let res = opts.open(&tmp).and_then(|mut f| f.write_all(data).and_then(|_| f.sync_all())).and_then(|_| std::fs::rename(&tmp, path));
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res.map_err(|e| format!("Couldn't save {} ({e}).", path.display()))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A server's files in a fresh temporary home
    pub fn temp(id: &str) -> (crate::Paths, Files, Config) {
        let home = std::env::temp_dir().join(format!("ls-control-test-{}", random_hex(6)));
        let p = crate::Paths::new(home, id).unwrap();
        let c = Config { id: id.into(), game: "cstrike".into(), port: 27015, url: "https://status.example.com/control".into(), view: json!({"name": "Friday Frags"}) };
        make_private_dir(&p.control_dir()).unwrap();
        std::fs::write(p.control_dir().join("config.json"), serde_json::to_string(&c).unwrap()).unwrap();
        (p.clone(), Files::new(&p), c)
    }

    /// The secret an invite carries
    pub fn secret_of(invite: &str) -> String {
        let body = invite.strip_prefix(INVITE_PREFIX).unwrap();
        let t: InviteText = serde_json::from_slice(&base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(body).unwrap()).unwrap();
        t.c
    }

    #[test]
    fn invites_work_once() {
        let (p, f, c) = temp("friday");
        let (_, text) = f.invite(&c, Role::Moderator, "", "owner").unwrap();
        let secret = secret_of(&text);
        let (m, key) = f.redeem(&secret, "Bob\n").unwrap();
        assert_eq!((m.role, m.name.as_str(), m.server.as_str()), (Role::Moderator, "Bob", "friday"));
        assert_eq!(key.len(), 64);
        assert!(f.redeem(&secret, "Eve").is_err(), "single use");
        // only the hash is kept
        let saved = std::fs::read_to_string(p.control_dir().join("store.json")).unwrap();
        assert!(!saved.contains(&key) && !saved.contains(&secret) && saved.contains(&sha(&key)));
        assert_eq!(f.member_for(&key).unwrap().unwrap().id, m.id);
        assert!(f.member_for(&sha(&key)).unwrap().is_none(), "the stored hash isn't a key");
        assert!(f.member_for("").unwrap().is_none() && f.member_for(&"z".repeat(64)).unwrap().is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(p.control_dir().join("store.json")).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(std::fs::metadata(p.control_dir()).unwrap().permissions().mode() & 0o777, 0o700);
        }
        let _ = std::fs::remove_dir_all(&p.home);
    }

    #[test]
    fn invites_run_out() {
        let (p, f, c) = temp("friday");
        let (_, text) = f.invite(&c, Role::Admin, "STEAM_0:1:5", "owner").unwrap();
        f.edit(|s| {
            s.invites[0].expires = now() - 1;
            Ok(())
        })
        .unwrap();
        assert!(f.redeem(&secret_of(&text), "Bob").is_err(), "a day old");
        // an invite for another server (a store copied over) isn't this one's
        let (_, text) = f.invite(&c, Role::Admin, "", "owner").unwrap();
        f.edit(|s| {
            s.invites.iter_mut().for_each(|i| i.server = "other".into());
            Ok(())
        })
        .unwrap();
        assert!(f.redeem(&secret_of(&text), "Bob").is_err(), "pinned to its server");
        assert!(f.invite(&c, Role::Admin, "bob", "owner").is_err(), "not a SteamID");
        assert!(f.invite(&Config { url: String::new(), ..c.clone() }, Role::Admin, "", "owner").is_err(), "no https address yet");
        assert!(f.redeem("nonsense", "x").is_err());
        let _ = std::fs::remove_dir_all(&p.home);
    }

    #[test]
    fn removing_an_admin_takes_their_moderators() {
        let (p, f, c) = temp("friday");
        let (_, a) = f.invite(&c, Role::Admin, "", "owner").unwrap();
        let (admin, _) = f.redeem(&secret_of(&a), "Ann").unwrap();
        let (_, m) = f.invite(&c, Role::Moderator, "", &admin.id).unwrap();
        let (_, key) = f.redeem(&secret_of(&m), "Mo").unwrap();
        f.invite(&c, Role::Moderator, "", &admin.id).unwrap();
        let gone = f.remove(&admin.id).unwrap();
        assert_eq!(gone.len(), 2);
        assert!(f.member_for(&key).unwrap().is_none());
        assert!(f.read().unwrap().invites.is_empty(), "their open invites end too");
        let v = f.view("friday", 10).unwrap();
        assert!(v["members"].as_array().unwrap().is_empty());
        assert!(!v.to_string().contains("\"key\""));
        let _ = std::fs::remove_dir_all(&p.home);
    }

    #[test]
    fn log_hides_secrets() {
        let (p, f, c) = temp("friday");
        let (_, a) = f.invite(&c, Role::Admin, "", "owner").unwrap();
        let (m, _) = f.redeem(&secret_of(&a), "Ann").unwrap();
        f.log_change(&m, "/api/cmd", Some(&json!({"cmd": "gm_set", "rcon_password": "x", "args": ["a"]})));
        let a = f.activity(10);
        assert_eq!(a[0]["body"]["rcon_password"], "…");
        assert_eq!(a[0]["can_undo"], false);
        assert!(stamp().len() == 15 && ct_eq("ab", "ab") && !ct_eq("ab", "ac") && !ct_eq("a", "ab"));
        let _ = std::fs::remove_dir_all(&p.home);
    }
}
