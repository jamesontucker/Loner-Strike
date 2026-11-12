import { useTools, type HeatMode, type KnowMode, type Layers as LayerSet, type NavColour } from "../../../stores/tools";
import { Info, Switch } from "../../../ui";
import type { Overlays } from "../MapViewer";

const ROWS: [keyof LayerSet, string, string][] = [
  ["textures", "Textures", "The map's own pictures on every surface. Off: a plain colour per texture, easier on the eyes when you're looking at shapes."],
  ["lighting", "Lighting", "The light and shadow baked into the map, as in the game. Off: everything evenly lit."],
  ["entities", "Spawns & items", "Spawn points (T orange, CT blue), lights, items, sounds and other markers."],
  ["moving", "Doors & breakables", "Doors, breakable glass and boxes, ladders, lifts and other moving parts."],
  ["triggers", "Triggers & zones", "Invisible areas: buy zones, bomb sites, hostage rescue, and triggers that hurt, teleport or start something."],
];

const KNOW: [KnowMode, string][] = [
  ["off", "Nothing"],
  ["cover", "Cover"],
  ["sight", "What they see"],
  ["bangs", "Wallbangs"],
  ["corners", "Corners"],
  ["angles", "Angles they watch"],
];
const KNOW_HELP: Record<KnowMode, string> = {
  off: "",
  cover: "Green areas are sheltered, red ones open. Pick an area for how far it's open in 16 directions (yellow crouched, blue standing).",
  sight: "Areas tinted by how much they see. Pick an area for lines to everything visible from it (yellow: only when crouched).",
  bangs: "Walls bots learned to shoot through, coloured by material. Pick an area to see only its own.",
  corners: "Corners bots peek around, with the way each opens.",
  angles: "Pick an area for the angles bots watch from it; brighter = more important.",
};

export function Layers({ overlays }: { overlays: Overlays }) {
  const layers = useTools((s) => s.layers);
  const setLayer = useTools((s) => s.setLayer);
  const navColour = useTools((s) => s.navColour);
  const know = useTools((s) => s.know);
  const heat = useTools((s) => s.heat);
  const { setNavColour, setKnow, setHeat } = useTools.getState();
  const nav = overlays.nav;
  return (
    <section className="tools-panel" aria-label="Layers">
      <h3>Show</h3>
      {ROWS.map(([k, label, info]) => (
        <div className="setting" key={k}>
          <span className="name">
            {label}
            <Info text={info} />
          </span>
          <Switch on={layers[k]} label={label} onChange={(v) => setLayer(k, v)} />
        </div>
      ))}
      <div className="setting">
        <span className="name">
          Bots' nav
          <Info text="The map of walkable areas bots use to get around. Areas no other area leads to (islands) show in red; one-way links in orange." />
        </span>
        <Switch on={layers.nav} label="Bots' nav" onChange={(v) => setLayer("nav", v)} />
      </div>
      {layers.nav && (
        <div className="tools-sub">
          {overlays.navLoading && <p className="small-note muted"><span className="spinner" aria-hidden="true" />Reading the nav…</p>}
          {overlays.navError && <p className="small-note tools-warn">{overlays.navError}</p>}
          {nav?.warning && <p className="small-note tools-warn">{nav.warning}</p>}
          {nav?.matchesMap === false && <p className="small-note tools-warn">This nav was made for another version of the map, so areas may not line up.</p>}
          {nav && (
            <>
              <p className="small-note muted">
                {nav.areas.length} areas{nav.islands > 1 ? `, ${nav.islands - 1} cut off from the rest` : ""}{nav.oneWay.length ? `, ${nav.oneWay.length} one-way links` : ""}{nav.ladders.length ? `, ${nav.ladders.length} ladders` : ""}.
              </p>
              <label className="tools-field">
                <span>Colour by</span>
                <select value={navColour} onChange={(e) => setNavColour(e.target.value as NavColour)}>
                  <option value="island">Connected or cut off</option>
                  <option value="place" disabled={!nav.places.length}>Place names{nav.places.length ? "" : " (none)"}</option>
                  <option value="moves">Crouch and jump spots</option>
                </select>
              </label>
              <label className="tools-field">
                <span>What bots learned</span>
                <select value={know} onChange={(e) => setKnow(e.target.value as KnowMode)}>
                  {KNOW.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
                </select>
              </label>
              {know !== "off" && (
                <>
                  {overlays.knowLoading && <p className="small-note muted"><span className="spinner" aria-hidden="true" />Reading what bots learned…</p>}
                  {overlays.knowError && <p className="small-note tools-warn">{overlays.knowError}</p>}
                  {overlays.know?.warning && <p className="small-note tools-warn">{overlays.know.warning}</p>}
                  {overlays.know && !overlays.know.warning && overlays.know.count !== nav.areas.length && <p className="small-note tools-warn">Knowledge was built for a different nav. Play the map again with bots to rebuild it.</p>}
                  {overlays.know && !overlays.know.done && <p className="small-note muted">Still learning (stage: {overlays.know.stageName}).</p>}
                  <p className="small-note muted">{KNOW_HELP[know]}</p>
                </>
              )}
            </>
          )}
        </div>
      )}
      {overlays.replay && (
        <label className="tools-field">
          <span>Heat map<Info text="Where players spent their time, died, or got stuck, in 128-unit squares laid over the map. Clearest in the Top-down view." /></span>
          <select value={heat} onChange={(e) => setHeat(e.target.value as HeatMode)} disabled={!overlays.replay.heat}>
            <option value="off">Off</option>
            <option value="time">Time spent</option>
            <option value="deaths">Deaths</option>
            <option value="stuck">Stuck</option>
          </select>
        </label>
      )}
    </section>
  );
}
