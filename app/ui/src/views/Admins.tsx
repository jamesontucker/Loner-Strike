import { useEffect, useState } from "react";
import { getJson, postJson } from "../api";
import type { Online } from "../types";
import { SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";

type Role = "owner" | "admin" | "moderator" | "vip";
type Person = { id: string; name: string; role: Role; expires: string | null; note: string; expired: boolean };
type Perks = { armor: boolean; defuse: boolean; nades: boolean; money: number; tag: boolean; chat: boolean; host: boolean };
type AbilityKey = "gear" | "hook" | "jump" | "respawn" | "noclip" | "god" | "tp";
type Abilities = Record<AbilityKey, Role[]> & { every_mode: boolean };
type Ban = { kind: "id" | "ip"; value: string };
type Seen = { id: string; name: string; aka: string[]; first: number; last: number; visits: number };
type Data = { people: Person[]; donate: string; perks: Perks; abilities: Abilities; bans: Ban[]; today: string; users_ini: boolean; seen: Seen[] };
type Reply = Data | { ok: false; error: string };

const ROLES: { id: Role; name: string; what: string }[] = [
  { id: "owner", name: "Owner", what: "Everything, including server settings and RCON. Can't be kicked by other admins." },
  { id: "admin", name: "Admin", what: "Kick, ban, slay, change maps, votes, admin chat and menus. Plus the abilities you pick below." },
  { id: "moderator", name: "Moderator", what: "Kick, slay, votes, admin chat and menus. No bans or map changes." },
  { id: "vip", name: "VIP", what: "A reserved slot, the VIP tags and the abilities you pick below. No admin powers." },
];
const roleName = (r: Role) => ROLES.find((x) => x.id === r)?.name ?? r;

/** The ability grid's columns, lowest role first. */
const COLUMNS: Role[] = ["vip", "moderator", "admin", "owner"];

/** What each ability does and how players use it. unfair: off in modes it would break. */
const ABILITIES: { key: AbilityKey; name: string; what: string; how: string; unfair?: boolean }[] = [
  { key: "gear", name: "Spawn gear", what: "The armor, kit, grenades and money set up further down.", how: "Given automatically" },
  { key: "hook", name: "Grappling hook", what: "Shoots a rope and pulls them to wherever they aim.", how: "Bind a key to +hook" },
  { key: "jump", name: "Double jump", what: "One more jump in mid-air.", how: "Press jump again in the air", unfair: true },
  { key: "respawn", name: "Respawn", what: "Come back once a round after dying.", how: "Say /respawn while dead", unfair: true },
  { key: "noclip", name: "Noclip", what: "Fly through walls. Ends when they die or the round restarts.", how: "Say /noclip to switch it on and off", unfair: true },
  { key: "god", name: "God mode", what: "Nothing can hurt them. Ends when the round restarts.", how: "Say /god to switch it on and off", unfair: true },
  { key: "tp", name: "Save and teleport", what: "Save a spot and jump back to it any time that map.", how: "Say /save, then /tp" },
];

/** Admin powers per role (from the role's AMX Mod X flags; not changeable here). */
const POWERS: Record<Role, string> = {
  vip: "None",
  moderator: "Kick, slay, votes",
  admin: "Also bans and maps",
  owner: "Everything",
};

const BAN_TIMES: [number, string][] = [[0, "For good"], [30, "30 minutes"], [180, "3 hours"], [1440, "1 day"], [10080, "1 week"]];
const MONEY = [0, 300, 500, 1000, 2000];
const STEAMID = /^STEAM_[0-5]:[01]:\d{1,10}$/;

/** YYYY-MM-DD n days after day. */
function addDays(day: string, n: number) {
  const d = new Date(`${day}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + n);
  return d.toISOString().slice(0, 10);
}

function niceDate(day: string, today: string) {
  const d = new Date(`${day}T00:00:00Z`);
  const sameYear = day.slice(0, 4) === today.slice(0, 4);
  return d.toLocaleDateString([], { day: "numeric", month: "short", year: sameYear ? undefined : "numeric", timeZone: "UTC" });
}

function expiry(p: Person, today: string) {
  if (!p.expires) return "";
  if (p.expired) return "expired";
  return `${roleName(p.role)} until ${niceDate(p.expires, today)}`;
}

function ago(t: number) {
  const s = Date.now() / 1000 - t;
  if (s < 3600) return "just now";
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  if (s < 86400 * 30) return `${Math.floor(s / 86400)} d ago`;
  return new Date(t * 1000).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" });
}

/** What the "Give…" menu offers: a role, and for VIP how long. */
const GIVE: { key: string; label: string; role: Role; days?: number }[] = [
  { key: "vip30", label: "VIP for 30 days", role: "vip", days: 30 },
  { key: "vip90", label: "VIP for 90 days", role: "vip", days: 90 },
  { key: "vip", label: "VIP for good", role: "vip" },
  { key: "moderator", label: "Moderator", role: "moderator" },
  { key: "admin", label: "Admin", role: "admin" },
  { key: "owner", label: "Owner", role: "owner" },
];

type Draft = { id: string; name: string; role: Role; expires: string; note: string; was?: string };
const EMPTY: Draft = { id: "", name: "", role: "vip", expires: "", note: "" };

export function Admins({ data, installed }: { data: Online | null; installed?: string[] }) {
  const ask = useConfirm();
  const toast = useToast();
  const [d, setD] = useState<Data | null>(null);
  const [failed, setFailed] = useState("");
  const [draft, setDraft] = useState<Draft | null>(null);
  const [banId, setBanId] = useState("");
  const [banTime, setBanTime] = useState(0);
  const [donate, setDonate] = useState<string | null>(null);
  const [q, setQ] = useState("");

  const load = () =>
    getJson<Reply>("/api/admins")
      .then((r) => ("people" in r ? setD(r) : setFailed(r.error)))
      .catch(() => setFailed("Couldn't reach the app."));
  useEffect(() => {
    load();
  }, []);

  /** Posts a change; the answer is the new list. */
  const send = async (path: string, body: unknown, done?: string) => {
    const r = await postJson<Reply>(path, body).catch(() => null);
    if (!r) return toast("Couldn't reach the app.", true), false;
    if (!("people" in r)) return toast(r.error || "That didn't work.", true), false;
    setD(r);
    if (done) toast(done);
    return true;
  };

  if (failed) return <div className="stack"><SectionHead title="Admins" /><p className="muted">{failed}</p></div>;
  if (!d) return <Skeleton />;

  const save = async () => {
    if (!draft) return;
    const ok = await send("/api/admins/person", { ...draft, id: draft.id.trim(), expires: draft.expires || null }, "Saved. Takes effect right away in a running game.");
    if (ok) setDraft(null);
  };
  const remove = async (p: Person) => (await ask(`Take ${p.name || p.id} off the list?`, { ok: "Remove", danger: true })) && send("/api/admins/remove", { id: p.id }, "Removed");
  const grant = (pl: { steamid: string; name: string }, role: Role, days?: number) => {
    const existing = d.people.find((p) => p.id === pl.steamid);
    send(
      "/api/admins/person",
      { id: pl.steamid, name: existing?.name || pl.name, role, expires: days ? addDays(d.today, days) : null, note: existing?.note ?? "", was: existing?.id },
      role === "vip" ? `${pl.name} is a VIP for ${days} days` : `${pl.name} is now ${role === "admin" ? "an admin" : `a ${roleName(role).toLowerCase()}`}`,
    );
  };
  const ban = async (id: string, minutes: number, name?: string) => {
    const what = minutes ? BAN_TIMES.find((t) => t[0] === minutes)?.[1].toLowerCase() : "for good";
    if (!(await ask(`Ban ${name || id} ${minutes ? `for ${what}` : "for good"}? They're kicked if they're playing.`, { ok: "Ban", danger: true }))) return false;
    return send("/api/admins/ban", { id: id.trim(), minutes }, "Banned");
  };
  const setPerks = (p: Partial<Perks>) => send("/api/admins/settings", { donate: d.donate, perks: { ...d.perks, ...p }, abilities: d.abilities });
  const saveDonate = () => send("/api/admins/settings", { donate: donate ?? d.donate, perks: d.perks, abilities: d.abilities }, "Saved").then((ok) => ok && setDonate(null));
  const setAbilities = (a: Partial<Abilities>) => send("/api/admins/settings", { donate: d.donate, perks: d.perks, abilities: { ...d.abilities, ...a } });
  const toggle = (key: AbilityKey, role: Role, on: boolean) => {
    const now = d.abilities[key].filter((r) => r !== role);
    setAbilities({ [key]: on ? [...now, role] : now });
  };
  const noHook = !!installed && !installed.includes("addon-hook");

  // who's playing now first, then everyone who's been here
  const humans = data ? data.players.filter((p) => !p.bot && !p.host && !!p.steamid && STEAMID.test(p.steamid)) : [];
  const players = [
    ...humans.map((h) => {
      const s = d.seen.find((x) => x.id === h.steamid);
      return { id: h.steamid!, name: h.name, aka: s?.aka ?? [], last: s?.last ?? 0, visits: s?.visits ?? 1, online: true };
    }),
    ...d.seen.filter((x) => !humans.some((h) => h.steamid === x.id)).map((x) => ({ ...x, online: false })),
  ];
  const people = [...d.people].sort((a, b) => ROLES.findIndex((r) => r.id === a.role) - ROLES.findIndex((r) => r.id === b.role) || (a.name || a.id).localeCompare(b.name || b.id));

  return (
    <div className="stack narrow">
      <div>
        <SectionHead title="Admins" note="Give people admin powers or VIP perks on your server, like real servers do.">
          <button className="btn primary" onClick={() => setDraft({ ...EMPTY })}>Add someone</button>
        </SectionHead>
        {!d.users_ini && <p className="err-text">AMX Mod X isn't set up for this game yet, so these can't take effect. Run Setup from Settings first.</p>}
        {draft && <Editor draft={draft} setDraft={setDraft} today={d.today} onSave={save} />}
        <ul className="people">
          {people.map((p) => (
            <li key={p.id} className={p.expired ? "expired" : ""}>
              <div className="person">
                <span className="person-name">
                  {p.name || p.id}
                  <i className={`role-badge ${p.role}`}>{roleName(p.role)}</i>
                </span>
                <span className="person-sub">
                  <span className="mono">{p.id}</span>
                  {p.expires && <span className={p.expired ? "err-text" : ""}> · {expiry(p, d.today)}</span>}
                  {p.note && <span> · {p.note}</span>}
                </span>
              </div>
              <div className="row">
                {p.role === "vip" && <button className="btn" onClick={() => send("/api/admins/person", { ...p, expires: addDays(p.expired || !p.expires ? d.today : p.expires, 30), was: p.id }, "30 more days")}>+30 days</button>}
                <button className="btn" onClick={() => setDraft({ id: p.id, name: p.name, role: p.role, expires: p.expires ?? "", note: p.note, was: p.id })}>Edit</button>
                <button className="btn danger" onClick={() => remove(p)}>Remove</button>
              </div>
            </li>
          ))}
          {!people.length && <li className="empty">Nobody yet. As the host you already have every power in your own game.</li>}
        </ul>
        <p className="muted small-note">
          People are recognised by their SteamID. Admins open the admin menu with <b>amxmodmenu</b> in the console. When a VIP's time runs out they go back to
          being a normal player.
        </p>
      </div>

      <div>
        <SectionHead title="Players" note="Everyone who's joined, newest first. Give someone a role, or ban them, in one click." />
        <input className="search" type="search" placeholder="Search names or SteamIDs" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search players" />
        <ul className="people">
          {players
            .filter((pl) => !q.trim() || `${pl.name} ${pl.aka.join(" ")} ${pl.id}`.toLowerCase().includes(q.trim().toLowerCase()))
            .slice(0, 200)
            .map((pl) => {
              const listed = d.people.find((p) => p.id === pl.id);
              const banned = d.bans.some((b) => b.value === pl.id);
              return (
                <li key={pl.id}>
                  <div className="person">
                    <span className="person-name">
                      {pl.name}
                      {pl.online && <i className="role-badge online">Playing now</i>}
                      {listed && <i className={`role-badge ${listed.role}`}>{roleName(listed.role)}</i>}
                      {banned && <i className="role-badge banned">Banned</i>}
                    </span>
                    <span className="person-sub">
                      <span className="mono">{pl.id}</span>
                      {pl.last ? ` · ${pl.online ? "joined" : "last seen"} ${ago(pl.last)} · ${pl.visits} visit${pl.visits === 1 ? "" : "s"}` : ""}
                      {pl.aka.length > 0 && ` · also ${pl.aka.slice(0, 3).join(", ")}`}
                    </span>
                  </div>
                  <div className="row">
                    <select
                      value=""
                      aria-label={`Give ${pl.name} a role`}
                      onChange={(e) => {
                        const g = GIVE.find((x) => x.key === e.target.value);
                        if (g) grant({ steamid: pl.id, name: pl.name }, g.role, g.days);
                      }}
                    >
                      <option value="">{listed ? "Change role…" : "Give…"}</option>
                      {GIVE.map((g) => <option key={g.key} value={g.key}>{g.label}</option>)}
                    </select>
                    {!banned && <button className="btn danger" onClick={() => ban(pl.id, 0, pl.name)}>Ban</button>}
                  </div>
                </li>
              );
            })}
          {!players.length && <li className="empty">Nobody has joined yet. People show up here the first time they connect.</li>}
        </ul>
        <p className="muted small-note">
          Only people joining through Steam have a SteamID. On a LAN or offline game they show up as STEAM_ID_LAN and can't be listed, given roles or banned here.
        </p>
      </div>

      <div>
        <SectionHead title="Bans" note="People who can't join your server." />
        <div className="row ban-form">
          <input className="search" placeholder="SteamID, like STEAM_0:1:12345" value={banId} onChange={(e) => setBanId(e.target.value)} aria-label="SteamID to ban" />
          <select value={banTime} onChange={(e) => setBanTime(Number(e.target.value))} aria-label="How long">
            {BAN_TIMES.map(([m, l]) => <option key={m} value={m}>{l}</option>)}
          </select>
          <button className="btn danger" disabled={!STEAMID.test(banId.trim())} onClick={() => ban(banId, banTime).then((ok) => ok && setBanId(""))}>Ban</button>
        </div>
        <ul className="people">
          {d.bans.map((b) => {
            const who = d.people.find((p) => p.id === b.value);
            return (
              <li key={b.kind + b.value}>
                <div className="person">
                  <span className="person-name">{who?.name || <span className="mono">{b.value}</span>}</span>
                  <span className="person-sub">{b.kind === "ip" ? "IP address" : "SteamID"}{who ? <> · <span className="mono">{b.value}</span></> : null} · for good</span>
                </div>
                <button className="btn" onClick={async () => (await ask(`Unban ${b.value}?`, { ok: "Unban" })) && send("/api/admins/unban", b, "Unbanned")}>Unban</button>
              </li>
            );
          })}
          {!d.bans.length && <li className="empty">Nobody is banned for good.</li>}
        </ul>
        <p className="muted small-note">Bans for a set time last until they run out or the game restarts, so they aren't listed here.</p>
      </div>

      <div>
        <SectionHead title="What each role gets" note="Tick the abilities each role can use in the game. Changes reach a running game straight away." />
        <div className="ability-wrap">
          <table className="ability-grid">
            <thead>
              <tr>
                <th scope="col">Ability</th>
                {COLUMNS.map((r) => <th key={r} scope="col">{roleName(r)}</th>)}
              </tr>
            </thead>
            <tbody>
              {ABILITIES.map((a) => (
                <tr key={a.key}>
                  <th scope="row">
                    <span className="ability-name">{a.name}{a.unfair && <i className="role-badge" title="Off in modes it would break, unless you allow it below">Some modes</i>}</span>
                    <span className="ability-what">{a.what}</span>
                    <span className="ability-how">{a.how}</span>
                    {a.key === "hook" && noHook && <span className="ability-how err-text">The hook add-on isn't installed, so this does nothing yet. Add it from Mods.</span>}
                  </th>
                  {COLUMNS.map((r) => (
                    <td key={r}>
                      <input
                        type="checkbox"
                        checked={d.abilities[a.key].includes(r)}
                        onChange={(e) => toggle(a.key, r, e.target.checked)}
                        aria-label={`${a.name} for ${roleName(r)}`}
                      />
                    </td>
                  ))}
                </tr>
              ))}
              <tr className="ability-powers">
                <th scope="row">
                  <span className="ability-name">Admin powers</span>
                  <span className="ability-what">Kick, ban, change maps and so on. Set by the role itself; admins open the menu with amxmodmenu in the console.</span>
                </th>
                {COLUMNS.map((r) => <td key={r}>{POWERS[r]}</td>)}
              </tr>
            </tbody>
          </table>
        </div>
        <div className="settings">
          <Perk
            label="Allow in every mode"
            note="Double jump, respawn, noclip and god mode are normally off in Zombie, Hide and Seek, Deathrun, Kreedz, Knife Arena, Base Builder, Soccer Jam, Paintball and Tour of Duty, where they'd spoil the game."
            on={d.abilities.every_mode}
            set={(v) => setAbilities({ every_mode: v })}
          />
        </div>
        <p className="muted small-note">
          A player counts as their highest role. Bots never get abilities. You, the host, count as an owner only with "You get the perks too" switched on below. Players
          can say <b>/vip</b> in the game to see what they can use.
        </p>
      </div>

      <div>
        <SectionHead title="VIP & donations" note="What spawn gear gives, and where people can support your server." />
        <div className="panel vip-note">
          <p>
            Loner-Strike doesn't take payments. If you want donations, set up a page with Ko-fi, PayPal, Patreon or similar, put the link below, and players can say{" "}
            <b>/donate</b> in the game to see it. When someone donates, give them VIP here, for as long as you like.
          </p>
        </div>
        <div className="settings">
          <div className="setting">
            <span className="name">Donate link<span className="note">Shown when someone says /donate. Leave empty for none.</span></span>
            <div className="row">
              <input className="search" placeholder="https://ko-fi.com/yourname" value={donate ?? d.donate} onChange={(e) => setDonate(e.target.value)} aria-label="Donate link" />
              {donate !== null && donate !== d.donate && <button className="btn primary" onClick={saveDonate}>Save</button>}
            </div>
          </div>
          <Perk label="Free armor + helmet" note="Every spawn." on={d.perks.armor} set={(v) => setPerks({ armor: v })} />
          <Perk label="Defuse kit" note="For VIPs on the CT side." on={d.perks.defuse} set={(v) => setPerks({ defuse: v })} />
          <Perk label="Free grenades" note="HE, flash and smoke each spawn." on={d.perks.nades} set={(v) => setPerks({ nades: v })} />
          <div className="setting">
            <span className="name">Extra money<span className="note">Added at the start of each round.</span></span>
            <div className="seg" role="group" aria-label="Extra money">
              {MONEY.map((m) => (
                <button key={m} type="button" aria-pressed={d.perks.money === m} onClick={() => setPerks({ money: m })}>{m ? `$${m}` : "Off"}</button>
              ))}
            </div>
          </div>
          <Perk label="VIP on the scoreboard" note="The scoreboard's VIP column." on={d.perks.tag} set={(v) => setPerks({ tag: v })} />
          <Perk label="[VIP] chat tag" note="A green tag in front of their name in chat." on={d.perks.chat} set={(v) => setPerks({ chat: v })} />
          <Perk label="You get the perks too" note="As the host you're counted as an owner, for spawn gear and the abilities above." on={d.perks.host} set={(v) => setPerks({ host: v })} />
        </div>
        <p className="muted small-note">
          To keep things fair, modes like Zombie, Hide and Seek, Deathrun, Kreedz and Knife Arena skip the spawn perks, and deathmatch and one-weapon modes only give
          armor and the kit. Changes reach a running game straight away; the mode check happens on the next map.
        </p>
      </div>
    </div>
  );
}

function Perk({ label, note, on, set }: { label: string; note: string; on: boolean; set: (v: boolean) => void }) {
  return (
    <div className="setting">
      <span className="name">{label}<span className="note">{note}</span></span>
      <Switch on={on} onChange={set} label={label} />
    </div>
  );
}

function Editor({ draft, setDraft, today, onSave }: { draft: Draft; setDraft: (d: Draft | null) => void; today: string; onSave: () => void }) {
  const set = (p: Partial<Draft>) => setDraft({ ...draft, ...p });
  const validId = STEAMID.test(draft.id.trim().toUpperCase());
  return (
    <div className="panel editor">
      <h3>{draft.was ? `Edit ${draft.name || draft.was}` : "Add someone"}</h3>
      <div className="editor-grid">
        <label className="field">
          <span>SteamID</span>
          <input className="search mono" placeholder="STEAM_0:1:12345" value={draft.id} onChange={(e) => set({ id: e.target.value })} />
          <small>
            They can find it by typing <b>status</b> in the game console, or look it up on steamid.io.
            {draft.id && !validId && <span className="err-text"> That doesn't look like a SteamID.</span>}
          </small>
        </label>
        <label className="field">
          <span>Name</span>
          <input className="search" placeholder="So you know who it is" value={draft.name} maxLength={32} onChange={(e) => set({ name: e.target.value })} />
        </label>
      </div>
      <div className="field">
        <span>Role</span>
        <div className="seg" role="group" aria-label="Role">
          {ROLES.map((r) => (
            <button key={r.id} type="button" aria-pressed={draft.role === r.id} onClick={() => set({ role: r.id })}>{r.name}</button>
          ))}
        </div>
        <small>{ROLES.find((r) => r.id === draft.role)?.what}</small>
      </div>
      <div className="field">
        <span>How long</span>
        <div className="row">
          <div className="seg" role="group" aria-label="How long">
            <button type="button" aria-pressed={!draft.expires} onClick={() => set({ expires: "" })}>For good</button>
            {[7, 30, 90, 365].map((n) => (
              <button key={n} type="button" aria-pressed={draft.expires === addDays(today, n)} onClick={() => set({ expires: addDays(today, n) })}>
                {n === 365 ? "1 year" : `${n} days`}
              </button>
            ))}
          </div>
          <input type="date" className="search date" value={draft.expires} min={today} onChange={(e) => set({ expires: e.target.value })} aria-label="Last day" />
        </div>
        <small>{draft.expires ? `Until the end of ${niceDate(draft.expires, today)}.` : "No end date."}</small>
      </div>
      <label className="field">
        <span>Note</span>
        <input className="search" placeholder="e.g. donated $5 on Ko-fi" value={draft.note} maxLength={120} onChange={(e) => set({ note: e.target.value })} />
      </label>
      <div className="row">
        <button className="btn primary" disabled={!validId} onClick={onSave}>Save</button>
        <button className="btn" onClick={() => setDraft(null)}>Cancel</button>
      </div>
    </div>
  );
}
