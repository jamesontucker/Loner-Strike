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
