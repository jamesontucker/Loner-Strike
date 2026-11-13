// Small line icons (24x24, stroke only) for the sidebar and the mode cards.

const P: Record<string, string> = {
  play: '<path d="M7 4.5v15l12-7.5z"/>',
  maps: '<path d="M3 6.5l6-2.5 6 2.5 6-2.5v13.5l-6 2.5-6-2.5-6 2.5z"/><path d="M9 4v13.5M15 6.5V20"/>',
  bots: '<rect x="5" y="8" width="14" height="11" rx="2"/><path d="M12 4v4M9 13h.01M15 13h.01M9.5 16.5h5"/>',
  match: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7v5l3 2"/>',
  fun: '<path d="M4 20l4.5-11.5 7 7z"/><path d="M13 3.5v2.5M19 6l-2 2M20.5 11.5H18M15.5 9.5l1-1M10 4.5l.5 1.5"/>',
  players: '<circle cx="9" cy="9" r="3.2"/><path d="M3.5 19c.8-3.2 3-5 5.5-5s4.7 1.8 5.5 5"/><circle cx="17" cy="8" r="2.5"/><path d="M15.5 13.5c2.3.2 4 1.8 4.6 4.5"/>',
  addons: '<path d="M9 3v4M15 3v4M6 7h12v5a6 6 0 0 1-12 0zM12 18v3"/>',
  friends: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3.2 3.6 3.2 14.4 0 18M12 3c-3.2 3.6-3.2 14.4 0 18"/>',
  // a clipboard with ticked lines: the round rules (time, money, friendly fire)
  rules: '<rect x="5" y="4.5" width="14" height="16.5" rx="1.5"/><path d="M9 3h6v3H9z"/><path d="M8.5 11l1.2 1.2L12 10M8.5 16l1.2 1.2L12 15M14 11.2h2.5M14 16.2h2.5"/>',
  // a box: mods you install (the plug is the live Add-ons page)
  mods: '<path d="M3.5 7.5L12 3l8.5 4.5v9L12 21l-8.5-4.5z"/><path d="M3.5 7.5L12 12l8.5-4.5M12 12v9"/>',
  skins: '<path d="M8.5 4L4 6.5l1.8 3.6L7.5 9.5V20h9V9.5l1.7.6L20 6.5 15.5 4c-.7 1.3-2 2-3.5 2s-2.8-.7-3.5-2z"/>',
  more: '<path d="M12 4v11M7.5 10.5L12 15l4.5-4.5M5 19h14"/>',
  settings: '<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>',
  classic: '<rect x="3.5" y="9" width="17" height="10" rx="1.5"/><path d="M8 9V6.5h8V9M7 13h4M7 16h6"/><circle cx="16.5" cy="14.5" r="1.6"/>',
  csdm: '<path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3"/><path d="M19.5 4v4.5H15"/><path d="M12 9v3l2 1.5"/>',
  csdm_ffa: '<path d="M5 4l9.5 9.5M19 4L9.5 13.5M13 15.5l1.5-1.5 4.5 4.5-1.5 1.5zM11 15.5L9.5 14 5 18.5 6.5 20z"/>',
  gungame: '<path d="M4 20h4v-4H4zM10 20h4v-9h-4zM16 20h4V4h-4z"/>',
  zp5: '<circle cx="12" cy="11" r="7"/><path d="M9 9l2 2M11 9l-2 2M13 9l2 2M15 9l-2 2M9 15h6"/>',
  war3: '<path d="M12 3l7 2.8V11c0 4.8-3 8.3-7 10-4-1.7-7-5.2-7-10V5.8z"/><path d="M12 7.5v9M8.5 11h7"/>',
  shero: '<path d="M13.5 2.5L5 14h6.5L10.5 21.5 19 10h-6.5z"/>',
  paintball: '<path d="M12 3.5c1.6 2.1 3.2 2.6 5.3 2.1-.5 2.1 0 3.7 2.2 5.3-2.2 1.6-2.7 3.2-2.2 5.3-2.1-.5-3.7 0-5.3 2.3-1.6-2.3-3.2-2.8-5.3-2.3.5-2.1 0-3.7-2.2-5.3 2.2-1.6 2.7-3.2 2.2-5.3 2.1.5 3.7 0 5.3-2.1z"/>',
  hns: '<circle cx="11" cy="11" r="6"/><path d="M15.5 15.5L21 21"/>',
  deathrun: '<path d="M3 19.5l3-9 3 9 3-9 3 9 3-9 3 9"/><path d="M2.5 19.5h19"/>',
  surf: '<path d="M2 19c3 0 3-2 6-2s3 2 6 2 3-2 6-2"/><path d="M5.5 14C8.5 8 13 4.5 20 4c-2.5 4-6 7.5-11.5 10z"/>',
  soccerjam: '<circle cx="12" cy="12" r="9"/><path d="M12 7l4 3-1.5 5h-5L8 10z"/>',
  basebuilder: '<rect x="3" y="5" width="18" height="14" rx="1"/><path d="M3 9.7h18M3 14.3h18M9 5v4.7M15 5v4.7M6 9.7v4.6M12 9.7v4.6M18 9.7v4.6M9 14.3V19M15 14.3V19"/>',
  zombie: '<circle cx="12" cy="11" r="7.5"/><path d="M9.3 9.5h.01M14.7 9.5h.01M8.8 15c2.1-1.2 4.3-1.2 6.4 0"/>',
  knife: '<path d="M3.5 20.5l6.5-6.5M10 14L20 4c.8 4.5-1.3 9.2-6.5 12.5z"/><path d="M7.5 15.5l1 1"/>',
  pistols: '<path d="M3 7h15v4h-6.5l-1 2.2H8l-1 4.8H3.8l1.4-7H3z"/>',
  deagle: '<path d="M2 7h19v4h-8l-1 2.2H9l-1 4.8H4.8l1.4-7H2z"/><path d="M18.5 7V5"/>',
  awp: '<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="1.6"/><path d="M12 3.5v5M12 15.5v5M3.5 12h5M15.5 12h5"/>',
  scoutz: '<path d="M12 21V8M7.5 12.5 12 8l4.5 4.5"/><path d="M5 4.5c4.5-1.8 9.5-1.8 14 0"/>',
  admins: '<path d="M12 3l7 2.8V11c0 4.8-3 8.3-7 10-4-1.7-7-5.2-7-10V5.8z"/><path d="M12 8l1.2 2.5 2.7.3-2 1.9.5 2.7-2.4-1.3-2.4 1.3.5-2.7-2-1.9 2.7-.3z"/>',
  browser: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="M4 10.5h13M10.5 4c2 2 2 11 0 13M10.5 4c-2 2-2 11 0 13M15.5 15.5L21 21"/>',
  server: '<rect x="4" y="4" width="16" height="6.5" rx="1"/><rect x="4" y="13.5" width="16" height="6.5" rx="1"/><path d="M7.5 7.25h.01M7.5 16.75h.01M11 7.25h5M11 16.75h5"/>',
  welcome: '<rect x="3.5" y="4.5" width="17" height="15" rx="1.5"/><path d="M3.5 8.5h17M7 12.5h10M7 15.5h6"/>',
  sound: '<path d="M4 9.5h3.5L12 5v14l-4.5-4.5H4z"/><path d="M15.5 9a4.5 4.5 0 0 1 0 6M18 6.5a8 8 0 0 1 0 11"/>',
  extras: '<path d="M4 6h16M4 12h16M4 18h10"/><circle cx="18" cy="18" r="2.5"/>',
  moderation: '<path d="M5 21V4"/><path d="M5 4h11l-2 4 2 4H5"/>',
  stats: '<path d="M4 20V10M10 20V4M16 20v-7M22 20H2"/>',
  menu: '<path d="M4 7h16M4 12h16M4 17h16"/>',
  // a wrench: tools for looking inside (and later fixing) maps
  tools: '<path d="M4.5 19.5l7.8-7.8"/><path d="M12.3 11.7a4.5 4.5 0 0 1 5.9-6.6l-2.7 2.7.3 2.4 2.4.3 2.7-2.7a4.5 4.5 0 0 1-6.6 5.9"/>',
  kreedz: '<path d="M3 21l6-10 4 5 3-4 5 9z"/><path d="M14 6l2-3 2 3"/>',
  // a closing circle round a parachute-ish drop marker: the zone
  br: '<circle cx="12" cy="12" r="9" stroke-dasharray="3 2"/><circle cx="12" cy="12" r="4.5"/><path d="M12 9.5v5M9.5 12h5"/>',
  // a wall of three blocks with a shield in front: hold the line
  laststand: '<path d="M3 20h18M4 20v-5h5v5M15 20v-5h5v5M9.5 15v-4h5v4"/><path d="M12 3.5l4 1.6v3c0 2.6-1.7 4.4-4 5.4-2.3-1-4-2.8-4-5.4v-3z"/>',
  // ---- one per tool (views/tools/registry.tsx), by its id: tool-<id> ----
  "tool-overview": '<rect x="3.5" y="3.5" width="7" height="7" rx="1"/><rect x="13.5" y="3.5" width="7" height="7" rx="1"/><rect x="3.5" y="13.5" width="7" height="7" rx="1"/><rect x="13.5" y="13.5" width="7" height="7" rx="1"/>',
  "tool-map": '<path d="M3 16.5l9-4 9 4-9 4z"/><path d="M4.5 8C6.5 5 9 3.8 12 3.8S17.5 5 19.5 8c-2 3-4.5 4.2-7.5 4.2S6.5 11 4.5 8z"/><circle cx="12" cy="8" r="1.7"/>',
  "tool-replay": '<circle cx="4.5" cy="18.5" r="1.5"/><path d="M6 18.5c3 0 3-5.5 6-5.5s2.5 4 6.5 4" stroke-dasharray="2.2 2.4"/><path d="M13 3.5v8l6.5-4z"/>',
  "tool-demos": '<rect x="3.5" y="10" width="17" height="10" rx="1"/><path d="M3.5 10l-.6-3.6 16.3-2.8.6 3.6"/><path d="M8.2 5.6l2.4 3.2M13.6 4.7l2.4 3.2"/>',
  "tool-health": '<path d="M12 20s-8-4.7-8-10.3A4.3 4.3 0 0 1 12 7.4a4.3 4.3 0 0 1 8 2.3C20 15.3 12 20 12 20z"/><path d="M6.5 12.5h3l1.5-2.5 2 4.5 1.5-2h3"/>',
  "tool-doctor": '<rect x="3.5" y="7.5" width="17" height="12.5" rx="1.5"/><path d="M9 7.5V5.5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/><path d="M12 10.8v6M9 13.8h6"/>',
  "tool-navfix": '<path d="M8.5 3.5h7v6h-7zM3 14.5h7v6H3zM14 14.5h7v6h-7z"/><path d="M7 14.5l3-5M17 14.5l-3-5"/><path d="M10.5 17.5h3" stroke-dasharray="1 2"/>',
  "tool-callouts": '<path d="M4 4.5h16v11h-8.5l-4.5 4v-4H4z"/><path d="M9.6 13l2.4-5.5 2.4 5.5M10.4 11.2h3.2"/>',
  "tool-assets": '<path d="M3.5 3.5h10v10h-10z"/><path d="M3.5 8.5h10M8.5 3.5v10"/><path d="M3.5 3.5h5v5h-5zM8.5 8.5h5v5h-5z" fill="currentColor" fill-opacity=".35" stroke="none"/><path d="M17 11.5l4 2.2v4.6L17 20.5l-4-2.2v-1"/><path d="M13 13.7l4 2.3 4-2.3M17 16v4.5"/>',
  "tool-editor": '<path d="M3.5 13l8.5-3.5 8.5 3.5-8.5 3.5z"/><path d="M3.5 13v4.5L12 21l8.5-3.5V13M12 16.5V21"/><path d="M12 7.5V2.5M9.8 4.5L12 2.5l2.2 2"/>',
  "tool-studio": '<rect x="2.5" y="4" width="8" height="6" rx="1.2"/><rect x="13.5" y="14" width="8" height="6" rx="1.2"/><path d="M10.5 7c5 0 0 10 3 10"/><circle cx="10.5" cy="7" r=".9" fill="currentColor"/><circle cx="13.5" cy="17" r=".9" fill="currentColor"/>',
  "tool-plugins": '<path d="M7.5 7L3 12l4.5 5M16.5 7L21 12l-4.5 5M13.5 4.5l-3 15"/>',
  "tool-creatures": '<path d="M12 20.5c-3.4 0-5.3-1.6-5.3-3.7 0-2.6 2.5-4.8 5.3-4.8s5.3 2.2 5.3 4.8c0 2.1-1.9 3.7-5.3 3.7z"/><circle cx="5" cy="10.5" r="1.8"/><circle cx="9" cy="6" r="1.9"/><circle cx="15" cy="6" r="1.9"/><circle cx="19" cy="10.5" r="1.8"/>',
  "tool-radar": '<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="4"/><path d="M12 12l6-6"/><path d="M15.6 15.2h.01" stroke-width="2.6"/>',
  "tool-spray": '<rect x="6.5" y="9" width="8" height="12" rx="1.5"/><path d="M8.5 9V6.5h4V9M10.5 6.5V4.5h2.5"/><path d="M17 4.5h.01M19.5 3h.01M19.5 6h.01M17.5 8h.01M21 4.5h.01" stroke-width="2.2"/><path d="M6.5 13.5h8"/>',
  "tool-playercfg": '<path d="M12 3v5.5M12 15.5V21M3 12h5.5M15.5 12H21"/><path d="M12 12h.01" stroke-width="2.6"/>',
  "tool-people": '<rect x="2.5" y="5" width="19" height="14" rx="1.5"/><circle cx="8.5" cy="10.5" r="2.2"/><path d="M5 16.5c.7-1.8 2-2.8 3.5-2.8s2.8 1 3.5 2.8"/><path d="M14.5 10h4M14.5 13.5h3"/>',
  "tool-community": '<circle cx="18" cy="5.5" r="2.5"/><circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="18.5" r="2.5"/><path d="M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1"/>',
};

export function Icon({ name, size = 20 }: { name: string; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      dangerouslySetInnerHTML={{ __html: P[name] ?? P.classic }}
    />
  );
}

export function Mark({ size = 30 }: { size?: number }) {
  return <img src="/logo.svg" width={size} height={size} alt="" />;
}
