// P5: a small link from a page or panel to its section of the docs (lonerstrike.com/docs#<id>),
// the same everywhere: "Docs" with an external-link icon opens it in the browser.

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

/** An arrow out of a box, drawn like the tool header's other icons (Focus, History). */
export const ExternalIcon = () => (
  <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" />
  </svg>
);

export function DocsLink({ to, what }: { to: string; what: string }) {
  return (
    <a className="btn small docs-link" href={`${DOCS_URL}#${encodeURIComponent(to)}`} target="_blank" rel="noreferrer" title={`Open the docs about ${what} in your browser`}>
      <ExternalIcon />
      Docs
    </a>
  );
}
