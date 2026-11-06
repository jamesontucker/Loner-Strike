"""Play with friends: shares the game over a playit.gg tunnel or Tailscale.

playit.gg gives the Mac a public address and relays your friends' game traffic
to it, so nothing on the router has to change. The panel runs playit's own
program (tools/playit/playitd, built from github.com/playit-cloud/playit-agent)
only while sharing is on, as a child of the panel: it never starts by itself.

Setup is a one-time approval on playit.gg; the agent key it hands back is kept
in tools/playit/secret.txt.

Tailscale is the other choice: a private network the host and their friends all
join. The panel only reads the Tailscale app's status for this Mac's address;
the app itself does the connecting.
"""

import json
import os
import re
import secrets
import shutil
import socket
import subprocess
import threading
import time
import urllib.error
import urllib.request

API = "https://api.playit.gg"
VERSION = "playit 1.2.0"
GAME_PORT = 27015
TUNNEL_NAME = "cz-server"

NAME_OK = re.compile(r"[^\w .,!?'()+\-:#@&/]", re.ASCII)
PASSWORD_OK = re.compile(r"^[A-Za-z0-9_\-]{0,24}$")

# playit's error codes, in words
ERRORS = {
    "RequiresPlayitPremium": "playit.gg wants a Premium account for this tunnel.",
    "PublicPortRequiresPlayitPremium": "playit.gg wants a Premium account for this tunnel.",
    "RegionRequiresPlayitPremium": "playit.gg wants a Premium account for this tunnel.",
    "RequiresVerifiedAccount": "Verify your email on playit.gg (check your inbox), then press Start sharing again.",
    "AgentVersionTooOld": "playit.gg says this copy of playit is too old. Run setup.py again to update it.",
    "AgentNotFound": "playit.gg doesn't know this Mac any more. Disconnect and set it up again.",
    "CodeExpired": "The setup link expired. Start again.",
    "UserRejected": "Setup was cancelled on playit.gg.",
}


TAILSCALE_PATHS = (
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    r"C:\Program Files\Tailscale\tailscale.exe",
)


def tailscale_status():
    """{"installed", "state", "ip", "name"} from the Tailscale app on this computer."""
    exe = shutil.which("tailscale") or next((p for p in TAILSCALE_PATHS if os.path.exists(p)), None)
    if not exe:
        return {"installed": False}
    try:
        out = subprocess.run([exe, "status", "--json"], capture_output=True, text=True, timeout=4).stdout
        d = json.loads(out)
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return {"installed": True, "state": "unknown"}
    me = d.get("Self") or {}
    ips = [ip for ip in (me.get("TailscaleIPs") or []) if "." in ip]
    return {"installed": True, "state": d.get("BackendState", ""),   # Running, Stopped, NeedsLogin...
            "ip": ips[0] if ips else None, "name": (me.get("DNSName") or "").rstrip(".") or None}


class PlayitError(Exception):
    pass


def call(path, body=None, key=None):
    req = urllib.request.Request(API + path, data=json.dumps(body or {}).encode(), method="POST")
    req.add_header("Content-Type", "application/json")
    req.add_header("User-Agent", "cz-control-panel")
    if key:
        req.add_header("Authorization", "Agent-Key " + key)
    try:
        with urllib.request.urlopen(req, timeout=15) as r:
            res = json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        try:
            res = json.loads(e.read() or b"{}")
        except ValueError:
            raise PlayitError("playit.gg answered with an error (%d)." % e.code)
    except (OSError, ValueError):
        raise PlayitError("Couldn't reach playit.gg. Check your internet connection.")
    if res.get("status") == "success":
        return res.get("data")
    code = res.get("data")
    if isinstance(code, dict):
        code = code.get("message") or code.get("type") or json.dumps(code)
    raise PlayitError(ERRORS.get(str(code), "playit.gg said: %s" % code))


class Sharing:
    def __init__(self, here, game):
        self.game = game
        self.dir = os.path.join(here, "tools", "playit")
        os.makedirs(self.dir, exist_ok=True)
        self.binary = os.path.join(self.dir, "playitd")
        self.secret_file = os.path.join(self.dir, "secret.txt")
        self.state_file = os.path.join(self.dir, "sharing.json")
        self.pid_file = os.path.join(self.dir, "playitd.pid")
        self.log_file = os.path.join(self.dir, "playitd.log")
        self.cfg_file = os.path.join(game.data, "friends.cfg")
        self.lock = threading.Lock()
        self.proc = None
        self.claim = None          # {"code", "url", "step", "error"} while setting up
        self.phase = "off"         # off, starting, online, error
        self.error = ""
        self.address = None        # {"host", "port", "ip"}
        self.account = ""
        self.saved = self._load()
        self._ts, self._ts_at = None, 0
        self._stop_leftover()

    # ---- saved state ----------------------------------------------------
    def _load(self):
        try:
            with open(self.state_file) as f:
                return json.load(f)
        except (OSError, ValueError):
            return {}

    def _save(self):
        with open(self.state_file, "w") as f:
            json.dump(self.saved, f, indent=1)

    def _key(self):
        try:
            with open(self.secret_file) as f:
                return f.read().strip() or None
        except OSError:
            return None

    def status(self):
        return {
            "installed": os.access(self.binary, os.X_OK),
            "connected": bool(self._key()),
            "claim": self.claim,
            "phase": self.phase,
            "error": self.error,
            "address": self.address,
            "account": self.account,
            "name": self.saved.get("name", ""),
            "password": self.saved.get("password", ""),
            "method": self.saved.get("method", "playit"),
            "tailscale": self.tailscale(),
            "port": GAME_PORT,
        }

    def tailscale(self):
        if time.time() - self._ts_at > 5:     # the page asks every 2 seconds
            self._ts, self._ts_at = tailscale_status(), time.time()
        return self._ts

    def set_method(self, method):
        if method not in ("playit", "tailscale"):
            raise ValueError("unknown method")
        if method == "tailscale":
            self.stop()       # one way at a time
        self.saved["method"] = method
        self._save()

    # ---- one-time setup -------------------------------------------------
    def connect(self):
        if self.claim and self.claim["step"] in ("waiting", "approve"):
            return
        code = secrets.token_hex(5)
        self.claim = {"code": code, "url": "https://playit.gg/claim/" + code, "step": "waiting", "error": ""}
        threading.Thread(target=self._claim, args=(self.claim,), daemon=True).start()

    def cancel(self):
        self.claim = None

    def _claim(self, claim):
        give_up = time.time() + 15 * 60
        try:
            while self.claim is claim:
                if time.time() > give_up:
                    raise PlayitError("The setup link expired. Start again.")
                step = call("/claim/setup", {"code": claim["code"], "agent_type": "self-managed", "version": VERSION})
                if step == "UserAccepted":
                    break
                if step == "UserRejected":
                    raise PlayitError(ERRORS["UserRejected"])
                claim["step"] = "approve" if step == "WaitingForUser" else "waiting"
                time.sleep(1.5)
            while self.claim is claim:
                try:
                    key = call("/claim/exchange", {"code": claim["code"]})["secret_key"]
                    break
                except PlayitError as e:
                    if "NotAccepted" not in str(e) and "NotSetup" not in str(e):
                        raise
                    time.sleep(2)
            else:
                return
            fd = os.open(self.secret_file, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
            with os.fdopen(fd, "w") as f:
                f.write(key + "\n")
            self.saved.pop("tunnel_id", None)
            self._save()
            claim["step"] = "done"
            self.claim = None
        except PlayitError as e:
            claim["step"], claim["error"] = "failed", str(e)

    def disconnect(self):
        self.stop()
        for path in (self.secret_file,):
            if os.path.exists(path):
                os.remove(path)
        self.saved.pop("tunnel_id", None)
        self._save()
        self.account = ""

    # ---- server name and password ----------------------------------------
    def set_server(self, name, password):
        name = NAME_OK.sub("", name or "")[:48].strip()
        password = (password or "").strip()
        if not PASSWORD_OK.match(password):
            raise ValueError("Passwords can use letters, numbers, - and _ (up to 24).")
        self.saved["name"], self.saved["password"] = name, password
        self._save()
        lines = []
        if name:
            lines.append('hostname "%s"' % name)
        lines.append('sv_password "%s"' % password)
        # The plugin runs this after every map change (see task_reapply_saved)
        with open(self.cfg_file, "w") as f:
            f.write("// Written by the panel's Friends tab\n" + "\n".join(lines) + "\n")
        try:
            for line in lines:
                self.game.rcon.send(line)
        except (OSError, ConnectionError, PermissionError):
            pass   # applied when the game next loads a map

    # ---- sharing --------------------------------------------------------
    def start(self):
        key = self._key()
        if not key:
            raise ValueError("Set up playit.gg first.")
        if not os.access(self.binary, os.X_OK):
            raise ValueError("playit isn't installed in tools/playit.")
        with self.lock:
            if self.phase in ("starting", "online"):
                return
            self.phase, self.error, self.address = "starting", "", None
        threading.Thread(target=self._run, args=(key,), daemon=True).start()

    def stop(self):
        with self.lock:
            self.phase, self.error, self.address = "off", "", None
            proc, self.proc = self.proc, None
        if proc and proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(5)
            except subprocess.TimeoutExpired:
                proc.kill()
        if os.path.exists(self.pid_file):
            os.remove(self.pid_file)

    def _fail(self, message):
        self.stop()
        self.phase, self.error = "error", message

    def _run(self, key):
        try:
            data = call("/v1/agents/rundata", key=key)
            perms = data.get("permissions") or {}
            self.account = perms.get("account_status", "")
            tunnel = self._find_tunnel(key)
            if not tunnel:
                created = call("/v1/tunnels/create", {
                    "ports": {"type": "custom-udp", "details": 1},
                    "origin": {"type": "agent", "data": {"agent_id": data["agent_id"], "config": {"fields": [
                        {"name": "local_ip", "value": "127.0.0.1"},
                        {"name": "local_port", "value": str(GAME_PORT)},
                    ]}}},
                    "enabled": True, "alloc": None, "name": TUNNEL_NAME, "firewall_id": None,
                }, key=key)
                self.saved["tunnel_id"] = created["id"] if isinstance(created, dict) else created
                self._save()
        except PlayitError as e:
            return self._fail(str(e))

        with self.lock:
            if self.phase != "starting":
                return
            self.proc = subprocess.Popen(
                [self.binary, "--secret-path", self.secret_file, "--socket-path", os.path.join(self.dir, "playitd.sock"),
                 "--log-path", self.log_file],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            proc = self.proc
        with open(self.pid_file, "w") as f:
            f.write(str(proc.pid))

        # Watch the tunnel until sharing stops: quickly until it has an address, then now and then
        while self.proc is proc:
            if proc.poll() is not None:
                return self._fail("playit stopped unexpectedly. Details are in tools/playit/playitd.log.")
            try:
                tunnel = self._find_tunnel(key)
            except PlayitError as e:
                tunnel, self.error = None, str(e)
            if tunnel:
                self._read_tunnel(tunnel)
            time.sleep(3 if self.phase == "starting" else 20)

    def _find_tunnel(self, key):
        tunnels = call("/v1/tunnels/list", key=key).get("tunnels", [])
        want = self.saved.get("tunnel_id")
        for t in tunnels:
            if t["id"] == want or (not want and t.get("name") == TUNNEL_NAME):
                if t["id"] != want:
                    self.saved["tunnel_id"] = t["id"]
                    self._save()
                return t
        return None

    def _read_tunnel(self, t):
        host = port = ip = None
        for a in t.get("connect_addresses", []):
            kind, v = a.get("type"), a.get("value", {})
            if kind == "addr4" and not ip:
                ip, port = v["address"].rsplit(":", 1)
            elif kind == "ip4" and not ip:
                ip, port = v["address"], str(v["default_port"])
            elif kind in ("auto", "domain") and not host:
                host = v["address"]
        if host and ":" in host:
            host, hport = host.rsplit(":", 1)
            port = port or hport
        if host and not ip:
            try:
                ip = socket.gethostbyname(host)
            except OSError:
                pass
        reasons = t.get("offline_reasons") or []
        if port and (ip or host):
            self.address = {"host": host, "ip": ip, "port": int(port)}
        if self.address and not reasons:
            self.phase, self.error = "online", ""
        elif reasons:
            self.error = {
                "PublicAllocationPending": "playit.gg is still picking an address...",
                "AgentOverLimit": "Your playit.gg account has too many agents. Remove an old one on playit.gg.",
                "AgentDisabled": "This Mac is turned off on playit.gg. Turn it back on there.",
                "TunnelDisabled": "The tunnel is turned off on playit.gg. Turn it back on there.",
            }.get(reasons[0], "playit.gg: " + reasons[0])

    def _stop_leftover(self):
        """Stops a playitd the panel started before it was closed without stopping it."""
        try:
            with open(self.pid_file) as f:
                pid = int(f.read().strip())
            out = subprocess.run(["ps", "-p", str(pid), "-o", "command="], capture_output=True, text=True).stdout
            if self.binary in out:
                os.kill(pid, 15)
            os.remove(self.pid_file)
        except (OSError, ValueError):
            pass
