// The spray maker: any picture becomes your spray in Counter-Strike 1.6 and Condition Zero.
// Choose a picture, see it on a wall the way the game draws it, install.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { BusyButton, LoadError, Segmented, Skeleton, Switch, useToast } from "../../../ui";
import { Check, Empty, Row, Rows } from "../common";
import { DEFAULT_CHOICE, installSpray, pickPicture, previewSpray, restoreSpray, sprayStatus, type Choice, type GameSpray, type SprayImage } from "./sprayApi";
import "./spray.css";
import { CommunitySprays } from "../community/Hooks";

const SIZE_NOTE: Record<Choice["size"], string> = {
  small: "About the size of the game's own sprays",
  medium: "A bit bigger",
  large: "The biggest the game allows",
};

export function SprayMaker() {
  const toast = useToast();
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["tools", "spray"], queryFn: sprayStatus });
  const [choice, setChoice] = useState<Choice>(DEFAULT_CHOICE);
  const [preview, setPreview] = useState<SprayImage | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [picking, setPicking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const [lock, setLock] = useState(true);
  const [skip, setSkip] = useState<string[]>([]);

  const picked = status.data?.picked ?? null;
  const games = status.data?.games ?? [];
  const set = (c: Partial<Choice>) => {
    setChoice((old) => ({ ...old, ...c }));
    setDone(null);
  };

  // the preview follows the choices (a short wait so dragging the colour picker doesn't flood)
  useEffect(() => {
    if (!picked) return;
    let live = true;
    setPreviewing(true);
    const t = setTimeout(async () => {
      const r = await previewSpray(choice);
      if (!live) return;
      setPreviewing(false);
      if (r.error || !r.png) {
        setPreviewError(r.error || "Couldn't make the spray.");
        setPreview(null);
      } else {
        setPreviewError(null);
        setPreview(r as SprayImage);
      }
    }, 120);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [choice, picked?.name, picked?.width, picked?.height]); // eslint-disable-line react-hooks/exhaustive-deps

  const pick = async () => {
    setPicking(true);
    const r = await pickPicture();
    setPicking(false);
    if (r.cancelled) return;
    if (r.error) return toast(r.error, true);
    setDone(null);
    await qc.invalidateQueries({ queryKey: ["tools", "spray"] });
  };

  const targets = games.filter((g) => !skip.includes(g.game)).map((g) => g.game);

  const install = async () => {
    setInstalling(true);
    const r = await installSpray(choice, targets, lock);
    setInstalling(false);
    if (r.error || !r.ok) return toast(r.error || "Couldn't install the spray.", true);
    const msg = r.message || "Spray installed. Press T in game to spray it.";
    setDone(msg);
    toast(msg);
    qc.invalidateQueries({ queryKey: ["tools", "spray"] });
  };

  const restore = async (g: GameSpray) => {
    setRestoring(g.game);
    const r = await restoreSpray(g.game);
    setRestoring(null);
    if (r.error || !r.ok) return toast(r.error || "Couldn't put your old spray back.", true);
    toast(r.message || "Your previous spray is back.");
    setDone(null);
    qc.invalidateQueries({ queryKey: ["tools", "spray"] });
  };

  if (status.isPending) return <Skeleton rows={4} />;
  if (status.isError) return <LoadError text="Couldn't load the spray maker." onRetry={() => status.refetch()} />;
  if (status.data?.error) return <LoadError text={status.data.error} onRetry={() => status.refetch()} />;
  if (games.length === 0)
    return (
      <Empty title="No game found" text="Couldn't find Counter-Strike 1.6 or Condition Zero on this computer. Set up the game first, then come back to make a spray." />
    );

  return (
    <div className="tl-page spray-page">
    <div className="spray">
      <div className="panel spray-stage">
        <div className="spray-wall" aria-label={preview ? "Your spray on a wall, as it looks in game" : undefined}>
          {preview ? (
            <img className="spray-img" src={preview.png} width={preview.width * 2} height={preview.height * 2} alt="Your spray as it looks in game" />
          ) : (
            <div className="spray-empty">
              <p>{picked ? (previewError ?? "Making your spray…") : "Choose a picture to see it here, sprayed on a wall."}</p>
            </div>
          )}
          {previewing && preview && <span className="spinner spray-busy" aria-label="Updating the preview" />}
        </div>
        <div className="row between spray-caption">
          <span className="muted">
            {picked ? (
              <>
                {picked.name}
                {preview && ` · ${preview.width}×${preview.height} in game · ${preview.colour ? `${preview.colours} colours` : "one colour"}`}
              </>
            ) : (
              "PNG, JPEG, BMP, TGA or WebP. A picture with a see-through background works best."
            )}
          </span>
          <BusyButton className={picked ? "btn" : "btn primary"} busy={picking} label={picked ? "Choose another picture" : "Choose a picture"} busyLabel="Choosing…" onClick={pick} />
        </div>
      </div>

      <div className="panel spray-options">
        <h3>Your spray</h3>
        <Rows>
          <Row name="Size" note={SIZE_NOTE[choice.size]}>
            <Segmented label="Size" values="small medium large" labels="Small|Medium|Large" current={choice.size} onPick={(v) => set({ size: v as Choice["size"] })} />
          </Row>
          <Row name="Colours" info="Full colour keeps the picture's own colours (up to 255). One colour paints it in a single colour, like the game's own sprays: dark parts are painted, light parts are left clear.">
            <Segmented label="Colours" values="colour mono" labels="Full colour|One colour" current={choice.look} onPick={(v) => set({ look: v as Choice["look"] })} />
          </Row>
          {choice.look === "mono" && (
            <>
              <Row name="Paint colour">
                <input className="spray-colour" type="color" value={choice.paint} aria-label="Paint colour" onChange={(e) => set({ paint: e.target.value })} />
              </Row>
              <Row name="Paint the light parts" note="For a light picture on a dark background">
                <Switch on={choice.invert} onChange={(v) => set({ invert: v })} label="Paint the light parts" />
              </Row>
            </>
          )}
          <Row
            name="See-through"
            info="Which parts of the picture the wall shows through. 'The picture's own' uses its transparency (PNG, WebP). 'Plain background' removes the colour in the top-left corner, for a logo on white. Space around a picture that isn't the spray's shape is always see-through."
          >
            <Segmented label="See-through" values="alpha background none" labels="The picture's own|Plain background|None" current={choice.clear} onPick={(v) => set({ clear: v as Choice["clear"] })} />
          </Row>
          {choice.look === "colour" && (
            <Row name="Smooth photo colours" note="Mixes colours with fine dots. Good for photos, not for logos">
              <Switch on={choice.dither} onChange={(v) => set({ dither: v })} label="Smooth photo colours" />
            </Row>
          )}
          {games.length > 1 && (
            <Row name="Install for">
              <div className="tl-checks">
                {games.map((g) => (
                  <Check
                    key={g.game}
                    label={g.label}
                    checked={!skip.includes(g.game)}
                    onChange={(on) => {
                      setSkip((s) => (on ? s.filter((x) => x !== g.game) : [...s, g.game]));
                      setDone(null);
                    }}
                  />
                ))}
              </div>
            </Row>
          )}
          <Row name="Keep it from being replaced" note="Picking a spray in the game's Options would replace yours. This makes the file read-only, like players usually do">
            <Switch on={lock} onChange={setLock} label="Keep it from being replaced" />
          </Row>
        </Rows>
        <div className="row spray-actions">
          <BusyButton className="btn primary" busy={installing} disabled={!preview || targets.length === 0} label="Install spray" busyLabel="Installing…" onClick={install} />
          {!picked ? <span className="muted">Choose a picture first.</span> : targets.length === 0 ? <span className="muted">Tick a game to install it for.</span> : null}
        </div>
        {done && (
          <p className="spray-done" role="status">
            {done} <span className="muted">If the game is open, it shows from the next server you join.</span>
          </p>
        )}
      </div>

      <div className="panel">
        <h3>Spray now</h3>
        <div className="spray-now">
          {games.map((g) => (
            <div className="spray-now-game" key={g.game}>
              <div className="spray-wall small">
                {g.current && !g.current.error ? (
                  <img className="spray-img" src={(g.current as SprayImage).png} alt={`Your spray in ${g.label}`} />
                ) : (
                  <span className="muted">{g.current?.error ? "Can't read it" : "None yet"}</span>
                )}
              </div>
              <div>
                <strong>{g.label}</strong>
                <p className="muted">{g.current && !g.current.error ? (g.readOnly ? "Read-only, so the game keeps it" : "The game may replace it from Options") : g.current?.error ? g.current.error : "The game makes one when you pick a spray in Options"}</p>
                {g.canRestore && <BusyButton className="btn small" busy={restoring === g.game} label="Put back the previous one" busyLabel="Putting back…" onClick={() => restore(g)} />}
              </div>
            </div>
          ))}
        </div>
      </div>
      <CommunitySprays />{/* (community: sprays installed from Tools > Community) */}
    </div>
    </div>
  );
}

export default SprayMaker;
