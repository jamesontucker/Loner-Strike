// The "Community" chip other pages show on things installed from Community (maps on the Maps and
// Play pages, modes on Play and Mods, prefabs in the editor, sprays in the spray maker): who made
// it, and a link to its page. Small on purpose: it's in the start bundle of those pages, so it
// imports nothing from the Community page itself.
//
//   const lib = useCommunity();
//   <CommunityChip chip={lib?.maps[name.toLowerCase()]} />
//   filter: isCommunityMap(lib, name)

import { useQuery } from "@tanstack/react-query";
import { getJson, ON_THIS_COMPUTER } from "../../../api";
import { useEffect } from "react";
import { useNav } from "../../../store";
import { useToast } from "../../../ui";

export type Chip = { key: string; name: string; author: string; url: string; kind: string };
export type CommunityLookup = { maps: Record<string, Chip>; modes: Record<string, Chip>; prefabs: Record<string, Chip>; pages: Record<string, Chip>; updates?: number; waiting?: number };

/** What's installed from Community (cached a minute; nothing on a phone, where Tools isn't). */
export function useCommunity(): CommunityLookup | undefined {
  const q = useQuery({
    queryKey: ["tools", "community", "library"],
    queryFn: () => getJson<CommunityLookup>("/api/tools/community/library"),
    enabled: ON_THIS_COMPUTER,
    staleTime: 60_000,
    retry: false,
  });
  return q.data && !(q.data as { error?: string }).error ? q.data : undefined;
}

export const communityMap = (lib: CommunityLookup | undefined, name: string) => lib?.maps[name.toLowerCase()];
export const communityMode = (lib: CommunityLookup | undefined, id: string) => lib?.modes[id];
export const communityPrefab = (lib: CommunityLookup | undefined, id: string) => lib?.prefabs[id];

const STYLE = { fontSize: 11, padding: "0 6px", whiteSpace: "nowrap" as const, textDecoration: "none", verticalAlign: 1 };

/** "Community · <author>", linking to its page. Renders nothing for things that aren't from Community. */
export function CommunityChip({ chip }: { chip?: Chip | null }) {
  if (!chip) return null;
  return (
    <a
      className="tag cm-chip"
      style={STYLE}
      href={chip.url}
      target="_blank"
      rel="noreferrer"
      title={`From Community: ${chip.name}${chip.author ? ` by ${chip.author}` : ""}. Opens its page.`}
      onClick={(e) => e.stopPropagation()}
    >
      Community{chip.author ? ` · ${chip.author}` : ""}
    </a>
  );
}

/** Opens Tools > Community on Browse, showing one kind ("map", "mode", "prefab", "spray",
 *  "texture"). Nothing on a phone, where Tools isn't. `link`: a text link instead of a button. */
export function CommunityLink({ kind, label, link, before }: { kind: string; label: string; link?: boolean; before?: () => void }) {
  if (!ON_THIS_COMPUTER) return null;
  return (
    <button
      type="button"
      className={link ? "link" : "btn small"}
      title="Opens Tools > Community: things other Loner-Strike players made, ready to try in a click"
      onClick={() => {
        before?.();
        useNav.getState().go("tools", `community:browse:${kind}`);
      }}
    >
      {label}
    </button>
  );
}

/** How many installed community creations have a newer version (0 when none or not known). */
export function useCommunityUpdates(): number {
  return useCommunity()?.updates ?? 0;
}

let told = 0;
/** Says once a session (and again when the count goes up) that community updates are waiting. */
export function useCommunityUpdateNotice(n: number) {
  const toast = useToast();
  useEffect(() => {
    if (n > told) {
      told = n;
      toast(`${n} community update${n === 1 ? "" : "s"} ready: Tools > Community > Installed.`);
    }
  }, [n, toast]);
}

/** A small count on a sidebar entry. */
export function CountBadge({ n, title }: { n: number; title: string }) {
  if (!n) return null;
  return (
    <span className="tag" title={title} aria-label={title} style={{ marginLeft: "auto", fontSize: 11, padding: "0 6px", background: "var(--signal)", color: "var(--signal-ink)" }}>
      {n}
    </span>
  );
}

// ---- origins (My library): made by me / from the community / remixed ----

export type Origin = { kind: "mine" | "community" | "remix" | "file"; author?: string; url?: string; name?: string; of?: string };
export type Origins = { maps: Record<string, Origin>; modes: Record<string, Origin>; prefabs: Record<string, Origin> };

/** Where each map, mode and prefab in the library came from (nothing on a phone). */
export function useOrigins(): Origins | undefined {
  const q = useQuery({
    queryKey: ["tools", "community", "origins"],
    queryFn: () => getJson<Origins>("/api/tools/community/origins"),
    enabled: ON_THIS_COMPUTER,
    staleTime: 60_000,
    retry: false,
  });
  return q.data && !(q.data as { error?: string }).error ? q.data : undefined;
}

/** The words for an origin: "Made by me", "Community · jam", "Remix of Lava", "From a file". */
export function originText(o?: Origin | null): string {
  if (!o) return "";
  switch (o.kind) {
    case "mine":
      return "Made by me";
    case "community":
      return `Community${o.author ? ` · ${o.author}` : ""}`;
    case "remix":
      return `Remix of ${o.of || "a community mode"}${o.author ? ` by ${o.author}` : ""}`;
    case "file":
      return "From a file";
  }
}

/** A small origin tag (not a link: safe inside a card's button). */
export function OriginTag({ o }: { o?: Origin | null }) {
  if (!o) return null;
  return (
    <span className="tag cm-chip" style={STYLE} title={o.kind === "community" ? `From Community: ${o.name ?? ""}${o.author ? ` by ${o.author}` : ""}` : originText(o)}>
      {originText(o)}
    </span>
  );
}

// ---- where a mode or mod came from: Built-in / Made by you / Community / Added ----------------

export type SourceKind = "builtin" | "mine" | "community" | "added";
export type Source = { kind: SourceKind; label: string; title: string };
export const SOURCE_LABEL: Record<SourceKind, string> = { builtin: "Built-in", mine: "Made by you", community: "Community", added: "Added" };

/** A mode's source: Community (the library knows the creator), your own Studio mode or remix, or
 *  else one that ships with Loner-Strike (nothing else records an origin for those). */
export function modeSource(id: string, lib: CommunityLookup | undefined, origins: Origins | undefined): Source {
  const cm = communityMode(lib, id);
  if (cm) return { kind: "community", label: `Community${cm.author ? ` · ${cm.author}` : ""}`, title: `From Community: ${cm.name}${cm.author ? ` by ${cm.author}` : ""}` };
  const o = origins?.modes[id];
  if (o?.kind === "community") return { kind: "community", label: originText(o), title: `From Community${o.name ? `: ${o.name}` : ""}` };
  if (o?.kind === "remix") return { kind: "mine", label: "Made by you", title: originText(o) };
  if (o?.kind === "mine") return { kind: "mine", label: "Made by you", title: "Made in Game Mode Studio" };
  if (o?.kind === "file") return { kind: "added", label: "Added", title: "Added from a file" };
  return { kind: "builtin", label: "Built-in", title: "Comes with Loner-Strike" };
}

/** The source as a small tag (not a link: safe inside a card's button). */
export function SourceBadge({ s }: { s: Source }) {
  return (
    <span className={`tag src-badge src-${s.kind}`} style={STYLE} title={s.title}>
      {s.label}
    </span>
  );
}

/** All / Built-in / Made by you / Community / Added, for a list with more than a handful in it;
 *  only the kinds the list has (and nothing when it has one kind). */
export function SourceFilter({ kinds, value, onChange, label }: { kinds: SourceKind[]; value: SourceKind | "all"; onChange: (k: SourceKind | "all") => void; label: string }) {
  const have = (["builtin", "mine", "community", "added"] as SourceKind[]).filter((k) => kinds.includes(k));
  if (have.length < 2) return null;
  return (
    <div className="chips src-filter" role="group" aria-label={label}>
      {(["all", ...have] as const).map((k) => (
        <button key={k} type="button" className="chip" aria-pressed={value === k} onClick={() => onChange(k)}>
          {k === "all" ? "All" : SOURCE_LABEL[k]}
        </button>
      ))}
    </div>
  );
}
