//! More ways for friends to reach the game:
//! - Direct: ask the router to forward the game's port (UPnP), the way consoles do
//! - Virtual LANs (ZeroTier, Radmin VPN, Hamachi): find this computer's address on them

use serde_json::{json, Value};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Mutex;
use std::time::Duration;

pub const GAME_PORT: u16 = 27015;
const DESCRIPTION: &str = "Loner-Strike (Counter-Strike)";
/// Leases expire on their own if the app is killed; renewed while sharing
const LEASE_SECS: u32 = 3600;

#[derive(Default)]
pub struct Direct {
    pub on: bool,
    pub external: Option<Ipv4Addr>,
    pub error: Option<String>,
    /// the router reports an address that isn't really public (shared/CGNAT or double NAT)
    pub shared_address: bool,
}

pub static DIRECT: Mutex<Direct> = Mutex::new(Direct { on: false, external: None, error: None, shared_address: false });

fn local_ipv4() -> Option<Ipv4Addr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("10.255.255.255:1").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

/// 10/8, 172.16/12, 192.168/16 or 100.64/10 (carrier-grade NAT): not reachable from the internet
fn not_public(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private() || (o[0] == 100 && (64..128).contains(&o[1])) || ip.is_loopback() || ip.is_link_local()
}

/// Opens the game port on the router (UDP 27015 → this computer). Blocking: call from a thread.
pub fn open_direct() -> Value {
    let result = (|| -> Result<(Ipv4Addr, bool), String> {
        let local = local_ipv4().ok_or("This computer isn't on a network right now.")?;
        let mut opts = igd_next::SearchOptions::default();
        opts.timeout = Some(Duration::from_secs(4));
        let gw = igd_next::search_gateway(opts)
            .map_err(|_| "Your router didn't answer. UPnP may be turned off in its settings, or it doesn't support it. Try playit.gg instead.".to_string())?;
        let external = match gw.get_external_ip() {
            Ok(IpAddr::V4(ip)) => ip,
            _ => return Err("Your router didn't say what its internet address is. Try playit.gg instead.".into()),
        };
        gw.add_port(
            igd_next::PortMappingProtocol::UDP,
            GAME_PORT,
            SocketAddr::V4(SocketAddrV4::new(local, GAME_PORT)),
            LEASE_SECS,
            DESCRIPTION,
        )
        .map_err(|e| format!("Your router refused to open the port ({e}). Try playit.gg instead."))?;
        Ok((external, not_public(external)))
    })();
    let mut d = DIRECT.lock().unwrap();
    match result {
        Ok((ip, shared)) => {
            *d = Direct { on: true, external: Some(ip), error: None, shared_address: shared };
        }
        Err(e) => {
            *d = Direct { on: false, external: None, error: Some(e), shared_address: false };
        }
    }
    drop(d);
    direct_status()
}

/// Closes the port again (sharing stopped, or the app quits).
pub fn close_direct() -> Value {
    let mut opts = igd_next::SearchOptions::default();
    opts.timeout = Some(Duration::from_secs(3));
    if let Ok(gw) = igd_next::search_gateway(opts) {
        let _ = gw.remove_port(igd_next::PortMappingProtocol::UDP, GAME_PORT);
    }
    *DIRECT.lock().unwrap() = Direct::default();
    direct_status()
}

/// Keeps the lease alive while sharing (the router drops it after LEASE_SECS otherwise).
pub fn renew_loop() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs((LEASE_SECS / 2) as u64));
        if DIRECT.lock().unwrap().on {
            open_direct();
        }
    });
}

pub fn direct_status() -> Value {
    let d = DIRECT.lock().unwrap();
    json!({
        "on": d.on,
        "address": d.external.map(|ip| format!("{ip}:{GAME_PORT}")),
        "error": d.error,
        "shared_address": d.shared_address,
    })
}

/// Addresses on virtual LAN apps, found by their network adapters:
/// Hamachi uses 25.x.x.x, Radmin VPN 26.x.x.x, ZeroTier names its adapters zt… (or "ZeroTier One" on Windows).
pub fn virtual_lans() -> Value {
    let mut out = serde_json::Map::new();
    for iface in if_addrs::get_if_addrs().unwrap_or_default() {
        let IpAddr::V4(ip) = iface.ip() else { continue };
        let name = iface.name.to_lowercase();
        let kind = if name.starts_with("zt") || name.contains("zerotier") {
            Some("zerotier")
        } else if ip.octets()[0] == 26 || name.contains("radmin") {
            Some("radmin")
        } else if ip.octets()[0] == 25 || name.contains("hamachi") || name.starts_with("ham") {
            Some("hamachi")
        } else {
            None
        };
        if let Some(k) = kind {
            out.entry(k).or_insert(json!(format!("{ip}:{GAME_PORT}")));
        }
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spots_addresses_the_internet_cant_reach() {
        assert!(not_public("100.72.1.9".parse().unwrap()));
        assert!(not_public("192.168.1.20".parse().unwrap()));
        assert!(!not_public("81.2.69.160".parse().unwrap()));
    }
}
