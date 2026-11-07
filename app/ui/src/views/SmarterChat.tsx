// Smarter chat (experimental): a small model on this computer writes bots' replies.
// The Bots page's section: models to download, the switch, and a Try it sheet.

import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { getJsonAt, ON_COMPUTER_ONLY, ON_THIS_COMPUTER, postJson, useActions } from "../api";
import { queryClient } from "../queries";
import { BusyButton, Info, SectionHead, Sheet, Switch, useConfirm, useToast } from "../ui";

type Model = { id: string; name: string; size: number; speed: string; good_at: string; credit: string; page: string; mode: string; downloaded: boolean };
type Download = { id: string; percent: number; done: boolean; error: string | null } | null;
type Status = { state: "off" | "waiting" | "starting" | "ready" | "failed"; reason: string | null; running: string | null; trial: boolean };
type ScInfo = { enabled: boolean; model: string | null; models: Model[]; supported: boolean; download: Download; status: Status; error?: string };
type Reply = { bot: string; persona: string; mood: string; text?: string | null; rejected?: string | null; ms?: number; error?: string };

const KEY = ["smartchat"];
const reload = () => queryClient.invalidateQueries({ queryKey: KEY });

function useSmartChat(fast: boolean) {
  return useQuery({
    queryKey: KEY,
    queryFn: ({ signal }) => getJsonAt<ScInfo>("/api/smartchat", null, signal),
    enabled: ON_THIS_COMPUTER,
    refetchInterval: fast ? 800 : 4000,
  });
}

const size = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.round(b / 1e6)} MB`);

function statusLine(s: Status, enabled: boolean): { text: string; tone: "" | "ok" | "err" } {
  if (s.state === "failed") return { text: `Not working. ${s.reason ?? ""} Bots use their written lines.`, tone: "err" };
  if (s.state === "ready") return { text: s.trial && !enabled ? "Ready for trying it out." : "Ready. Bots write their own replies.", tone: "ok" };
  if (s.state === "starting") return { text: "Starting…", tone: "" };
  if (s.state === "waiting") return { text: "On. It starts when you host a game.", tone: "" };
  return { text: "Off", tone: "" };
}

export function SmarterChat() {
  const toast = useToast();
  const ask = useConfirm();
  const { run: act, busy } = useActions();
  const [trying, setTrying] = useState(false);
  const anyBusy = busy("smartchat:dl");
  const q = useSmartChat(anyBusy || trying);
  const d = q.data;

  const head = (
    <SectionHead
      title="Smarter chat"
      note={
        <>
          <span className="tag">Experimental</span> Bots answer you in their own words.
          <Info text="When you talk to the bots, a small model on this computer writes their replies, and the short back-and-forths between them. Kills, round wins and other game events still use the written lines. Nothing leaves your computer. If the model is slow or writes something odd, the bot uses a written line instead." />
        </>
      }
    />
  );
  if (!ON_THIS_COMPUTER) {
    return (
      <section>
        {head}
        <p className="muted">{ON_COMPUTER_ONLY}</p>
      </section>
    );
  }
  if (!d) {
    return (
      <section>
        {head}
        <p className="muted">{q.isError ? "Couldn't load Smarter chat. Try again in a moment." : "Loading…"}</p>
      </section>
    );
  }
  if (!d.supported) {
    return (
      <section>
        {head}
        <p className="muted">Smarter chat doesn't run on this kind of computer yet.</p>
      </section>
    );
  }

  const have = d.models.filter((m) => m.downloaded);
  const dl = d.download;
  const status = statusLine(d.status, d.enabled);

  const set = (body: { enabled?: boolean; model?: string }, key: string, label: string) =>
    act(key, async () => {
      const r = await postJson<ScInfo>("/api/smartchat/set", body);
      if (r.error) toast(r.error, true);
      else queryClient.setQueryData(KEY, r);
    }, label);

  const download = (m: Model) =>
    act("smartchat:dl", async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/smartchat/download", { id: m.id });
      if (r.error) return toast(r.error, true);
      // follow it to the end (the page may close meanwhile; the download carries on)
      let misses = 0;
      for (;;) {
        await new Promise((res) => setTimeout(res, 800));
        let info: ScInfo;
        try {
          info = await getJsonAt<ScInfo>("/api/smartchat", null);
          misses = 0;
        } catch {
          if (++misses >= 10) return toast("Lost track of the download. Check back in a minute.", true);
          continue;
        }
        queryClient.setQueryData(KEY, info);
        const x = info.download;
        if (!x || x.id !== m.id) return;
        if (x.done) return x.error ? toast(x.error, true) : toast(`Downloaded ${m.name}`);
      }
    }, `Downloading ${m.name}…`);

  const remove = async (m: Model) => {
    if (!(await ask(`Remove ${m.name}? It frees ${size(m.size)}. You can download it again later.`, { ok: "Remove", danger: true }))) return;
    act(`smartchat:rm:${m.id}`, async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/smartchat/remove", { id: m.id });
      if (r.error) toast(r.error, true);
      else toast(`Removed ${m.name}`);
      await reload();
    }, `Removing ${m.name}…`);
  };

  return (
    <section>
      {head}
      <div className="panel stack-tight">
        <div className="row between setting-line toggle">
          <div>
            <b>Use Smarter chat</b>
            <p className="muted">{have.length ? "Bots use the model below when you host a game." : "Download a model first."}</p>
          </div>
          <Switch on={d.enabled} label="Use Smarter chat" disabled={!have.length || busy("smartchat:on")} onChange={(v) => set({ enabled: v }, "smartchat:on", v ? "Turning on Smarter chat…" : "Turning off Smarter chat…")} />
        </div>
        <div className="row between setting-line">
          <p className={`sc-status ${status.tone}`} aria-live="polite">
            {d.status.state === "starting" && <span className="spinner" aria-hidden="true" />}
            {status.text}
          </p>
          <button className="btn" disabled={!have.length} onClick={() => setTrying(true)}>
            Try it
          </button>
        </div>
        <ul className="sc-models" aria-label="Models">
          {d.models.map((m) => {
            const mine = dl && dl.id === m.id && !dl.done;
            const chosen = d.model === m.id && m.downloaded;
            return (
              <li key={m.id} className={`sc-model${chosen ? " chosen" : ""}`}>
                <label className="sc-pick">
                  <input
                    type="radio"
                    name="smartchat-model"
                    checked={chosen}
                    disabled={!m.downloaded || busy("smartchat:model")}
                    onChange={() => set({ model: m.id }, "smartchat:model", `Switching to ${m.name}…`)}
                  />
                  <span>
                    <b>{m.name}</b> <span className="muted">{size(m.size)} · {m.speed}</span>
                    {chosen && <span className="tag sc-using">In use</span>}
                    <span className="sc-good">{m.good_at}</span>
                    <small className="sc-credit">
                      {m.credit}{" "}
                      <a href={m.page} target="_blank" rel="noreferrer">
                        Model page
                      </a>
                    </small>
                  </span>
                </label>
                <div className="row sc-actions">
                  {m.downloaded ? (
                    <BusyButton className="btn danger small" busy={busy(`smartchat:rm:${m.id}`)} label="Remove" busyLabel="Removing…" onClick={() => remove(m)} />
                  ) : (
                    <BusyButton
                      className="btn small"
                      busy={!!mine}
                      disabled={anyBusy}
                      label="Download"
                      busyLabel={`Downloading… ${mine ? dl!.percent : 0}%`}
                      onClick={() => download(m)}
                    />
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      </div>
      {trying && <TryIt info={d} onClose={() => setTrying(false)} />}
    </section>
  );
}

/** Type a message, see what three sample bots would say, without a game running. */
function TryIt({ info, onClose }: { info: ScInfo; onClose: () => void }) {
  const toast = useToast();
  const have = info.models.filter((m) => m.downloaded);
  const [model, setModel] = useState(have.find((m) => m.id === info.model)?.id ?? have[0]?.id ?? "");
  const [text, setText] = useState("");
  const [replies, setReplies] = useState<Reply[] | null>(null);
  const [asking, setAsking] = useState(false);
  const field = useRef<HTMLInputElement>(null);

  // the server runs while the sheet is open (said again every 20 s), and stops when it closes
  useEffect(() => {
    let live = true;
    const start = () =>
      postJson<{ error?: string }>("/api/smartchat/try/start", { model })
        .then((r) => {
          if (r.error && live) toast(r.error, true);
          reload();
        })
        .catch(() => null);
    start();
    const t = window.setInterval(start, 20000);
    return () => {
      live = false;
      clearInterval(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [model]);
  useEffect(
    () => () => {
      postJson("/api/smartchat/try/stop", {})
        .catch(() => null)
        .then(reload);
    },
    [],
  );

  const s = info.status;
  const ready = s.state === "ready" && s.running === model;
  const failed = s.state === "failed";

  const send = async () => {
    const m = text.trim();
    if (!m || asking) return;
    setAsking(true);
    try {
      const r = await postJson<{ replies?: Reply[]; error?: string }>("/api/smartchat/try", { message: m });
      if (r.error) toast(r.error, true);
      else setReplies(r.replies ?? []);
    } catch {
      toast("Couldn't reach the app. Try again.", true);
    } finally {
      setAsking(false);
      field.current?.focus();
    }
  };

  return (
    <Sheet className="sheet wide" label="Try Smarter chat" onClose={onClose}>
      <div className="sheet-body stack-tight">
        <h2>Try Smarter chat</h2>
        <p className="muted">Say something the way you would in game. Three bots with different personalities answer.</p>
        {have.length > 1 && (
          <label className="row">
            Model
            <select value={model} onChange={(e) => setModel(e.target.value)} disabled={asking}>
              {have.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </select>
          </label>
        )}
        <p className={`sc-status ${failed ? "err" : ready ? "ok" : ""}`} aria-live="polite">
          {failed ? (
            `Not working. ${s.reason ?? ""}`
          ) : ready ? (
            "Ready"
          ) : (
            <>
              <span className="spinner" aria-hidden="true" />
              Loading the model…
            </>
          )}
        </p>
        <form
          className="row"
          onSubmit={(e) => {
            e.preventDefault();
            send();
          }}
        >
          <input
            ref={field}
            className="search full"
            maxLength={120}
            placeholder="are u a bot"
            aria-label="Your message"
            value={text}
            onChange={(e) => setText(e.target.value)}
            autoFocus
          />
          <BusyButton type="submit" className="btn primary" busy={asking} disabled={!ready || !text.trim()} label="Send" busyLabel="Asking…" />
        </form>
        {replies && (
          <div className="sc-replies">
            {replies.map((r) => (
              <div key={r.bot} className="sc-reply">
                <div className="sc-who">
                  <b>{r.bot}</b>
                  <small className="muted">
                    {r.persona}, {r.mood}
                  </small>
                </div>
                {r.text ? (
                  <p className="sc-line">{r.text}</p>
                ) : (
                  <p className="muted">
                    {r.error ?? "Nothing usable, so a written line would be used."}
                    {r.rejected && <small className="sc-rejected">It wrote: {r.rejected}</small>}
                  </p>
                )}
                {r.ms != null && <small className="muted">{(r.ms / 1000).toFixed(1)} s</small>}
              </div>
            ))}
          </div>
        )}
        <div className="row">
          <button className="btn" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </Sheet>
  );
}
