// P5: a small link from a page or panel to its section of the docs (lonerstrike.com/docs#<id>),
// the same everywhere: "Docs ↗" opens it in the browser.

export const DOCS_URL = "https://lonerstrike.com/docs";

/** Tools pages and their docs sections (the Studio's Help menu has its own). */
export const TOOL_DOCS: Record<string, string> = {
  editor: "map-editor",
  creatures: "creature-studio",
  plugins: "plugin-studio",
  doctor: "map-doctor",
  health: "map-simulator",
  navfix: "nav-fixer",
  callouts: "callouts",
  radar: "radar-maker",
  playercfg: "crosshair-config",
  people: "bot-people",
  spray: "spray-maker",
  map: "map-viewer",
  replay: "replay",
  demos: "demos",
  assets: "textures-models",
  community: "community",
};

export function DocsLink({ to, what }: { to: string; what: string }) {
  return (
    <a className="btn small docs-link" style={{ verticalAlign: "middle", marginLeft: 8, fontWeight: 600 }} href={`${DOCS_URL}#${encodeURIComponent(to)}`} target="_blank" rel="noreferrer" title={`Open the docs about ${what} in your browser`}>
      Docs <span aria-hidden="true">↗</span>
    </a>
  );
}
