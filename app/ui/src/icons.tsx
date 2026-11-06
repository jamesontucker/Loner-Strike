// Small line icons (24x24, stroke only) for the sidebar and the mode cards.

const P: Record<string, string> = {
  play: '<path d="M7 4.5v15l12-7.5z"/>',
  bots: '<rect x="5" y="8" width="14" height="11" rx="2"/><path d="M12 4v4M9 13h.01M15 13h.01M9.5 16.5h5"/>',
  match: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7v5l3 2"/>',
  fun: '<path d="M12 3l2.6 5.6 6 .7-4.5 4.1 1.3 6L12 16.4 6.6 19.4l1.3-6L3.4 9.3l6-.7z"/>',
  players: '<circle cx="9" cy="9" r="3.2"/><path d="M3.5 19c.8-3.2 3-5 5.5-5s4.7 1.8 5.5 5"/><circle cx="17" cy="8" r="2.5"/><path d="M15.5 13.5c2.3.2 4 1.8 4.6 4.5"/>',
  addons: '<path d="M9 3v4M15 3v4M6 7h12v5a6 6 0 0 1-12 0zM12 18v3"/>',
  friends: '<circle cx="8" cy="9" r="3"/><circle cx="16" cy="9" r="3"/><path d="M3 19c.6-2.8 2.6-4.5 5-4.5s4.4 1.7 5 4.5M11 19c.6-2.8 2.6-4.5 5-4.5s4.4 1.7 5 4.5"/>',
  more: '<path d="M12 4v11M7.5 10.5L12 15l4.5-4.5M5 19h14"/>',
  settings: '<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>',
  classic: '<circle cx="12" cy="13" r="7"/><path d="M12 6V3M9 3h6M17 8l2-2"/>',
  csdm: '<circle cx="12" cy="12" r="8"/><path d="M12 2v5M12 17v5M2 12h5M17 12h5"/>',
  csdm_ffa: '<path d="M12 2l2.5 6.5L21 9l-5 4.5L17.5 21 12 17l-5.5 4L8 13.5 3 9l6.5-.5z"/>',
  gungame: '<path d="M4 20h4v-4H4zM10 20h4v-9h-4zM16 20h4V4h-4z"/>',
  zp5: '<circle cx="12" cy="11" r="7"/><path d="M9 9l2 2M11 9l-2 2M13 9l2 2M15 9l-2 2M9 15h6"/>',
  war3: '<path d="M12 2l3 6 6 1-4.5 4 1 6.5L12 16l-5.5 3.5 1-6.5L3 9l6-1z"/><path d="M12 7v5"/>',
  shero: '<path d="M4 4h16l-3 7 3 9H4l3-9z"/><path d="M10 9h4l-2 5z"/>',
  paintball: '<circle cx="9" cy="9" r="4"/><circle cx="17" cy="15" r="3"/><circle cx="7" cy="18" r="2"/>',
  hns: '<circle cx="11" cy="11" r="6"/><path d="M15.5 15.5L21 21"/>',
  deathrun: '<path d="M4 20h16M6 20V10l6-6 6 6v10"/><path d="M10 20v-5h4v5"/>',
  surf: '<path d="M2 17c3 0 3-2 6-2s3 2 6 2 3-2 6-2M2 21c3 0 3-2 6-2s3 2 6 2 3-2 6-2M5 13c2-5 6-8 13-9-3 3-4 6-4 9"/>',
  soccerjam: '<circle cx="12" cy="12" r="9"/><path d="M12 7l4 3-1.5 5h-5L8 10z"/>',
  basebuilder: '<path d="M3 21h18M5 21V11h14v10M9 11V7h6v4"/>',
  kreedz: '<path d="M3 21l6-10 4 5 3-4 5 9z"/><path d="M14 6l2-3 2 3"/>',
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

export function Mark({ size = 26 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden="true">
      <rect x="64" y="64" width="896" height="896" rx="200" fill="#3A402F" />
      <circle cx="512" cy="512" r="250" fill="none" stroke="#E8792B" strokeWidth="64" />
      <path d="M512 150v230M512 644v230M150 512h230M644 512h230" stroke="#E9E3CF" strokeWidth="64" strokeLinecap="round" />
      <circle cx="512" cy="512" r="34" fill="#E8792B" />
    </svg>
  );
}
