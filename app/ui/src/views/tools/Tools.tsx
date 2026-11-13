// The Tools section: an overview of cards, or one tool (views/tools/registry.tsx), each
// loaded the first time it's opened. The sidebar lists the tools under Tools; every tool has
// an address: go("tools", "<id>") or go("tools", "<id>:<arg>"), go("tools", "overview") for
// the cards. The page itself loads lazily too, with three.js in its own chunk.

import { Suspense, useEffect } from "react";
import { ON_THIS_COMPUTER } from "../../api";
import { useNav } from "../../store";
import { useToolPage } from "../../stores/toolPage";
import { useTools } from "../../stores/tools";
import type { MapInfo } from "../../types";
import { lazyPage, SectionHead, Skeleton } from "../../ui";
import type { Vec3 } from "./viewer/coords";
import { TOOL_GROUPS, TOOLS } from "./registry";
import "./tools.css";
import "./common/common.css";

const PAGES = new Map(TOOLS.map((t) => [t.id, lazyPage(t.load)]));

/** Opens what a nav focus names: "overview", "map:de_dust2", "map:de_dust2@x,y,z[,size]",
 *  "replay:<id>", or "<tool>[:<arg>]". */
function openFocus(focus: string) {
  const at = focus.indexOf(":");
  const what = at < 0 ? focus : focus.slice(0, at);
  const arg = at < 0 ? "" : focus.slice(at + 1);
  const t = useTools.getState();
  if (what === "overview") return t.setTool("");
  if (what === "map") {
    t.setTool("map");
    if (!arg) return;
    const [name, spot] = arg.split("@");
    if (name !== t.map) t.openMap(name);
    const n = (spot ?? "").split(",").map(Number);
    // the viewer looks there once the map is up (Cameras' LookAt waits for it)
    if (n.length >= 3 && n.every(Number.isFinite)) t.look([n[0], n[1], n[2]] as Vec3, n[3] || 192);
  } else if (what === "replay") {
    t.setTool("replay");
    if (arg) t.openReplay(arg);
  } else if (PAGES.has(what)) {
    t.setTool(what);
    useTools.setState({ arg: arg || null });
  }
}

export default function Tools({ maps }: { maps: MapInfo[] }) {
  const focus = useNav((s) => s.focus);
  const tool = useToolPage((s) => s.tool);
  const entry = TOOLS.find((t) => t.id === tool);
  const Page = entry && PAGES.get(entry.id);

  // opened on a tool from the sidebar or elsewhere in the app
  useEffect(() => {
    if (!focus) return;
    openFocus(focus);
    useNav.getState().clearFocus();
  }, [focus]);

  if (!ON_THIS_COMPUTER)
    return (
      <>
        <SectionHead title="Tools" />
        <div className="panel">
          <h3>On the computer only</h3>
          <p className="muted measure">Tools work on the computer running Loner-Strike.</p>
        </div>
      </>
    );
  if (!entry || !Page) return <Overview />;
  return (
    <>
      <SectionHead
        title={entry.label}
        note={
          <>
            {entry.beta && <span className="tag tools-beta">Beta</span>} {entry.blurb}
          </>
        }
      />
      <Suspense fallback={<Skeleton rows={4} />}>
        <Page maps={maps} />
      </Suspense>
    </>
  );
}

/** Every tool as a card, grouped like the sidebar. */
function Overview() {
  const go = useNav((s) => s.go);
  return (
    <>
      <SectionHead
        title="Tools"
        note={
          <>
            <span className="tag tools-beta">Beta</span> New in 1.0: look inside maps, fix them for the bots, and make your own.
          </>
        }
      />
      {TOOL_GROUPS.map((g) => (
        <section key={g} className="tools-cards-group">
          <h3 className="tools-cards-head">{g}</h3>
          <div className="tools-cards">
            {TOOLS.filter((t) => t.group === g).map((t) => (
              <button key={t.id} className="tools-card" title={t.blurb} onClick={() => go("tools", t.id)}>
                <span className="tools-card-title">
                  {t.label}
                  {t.beta && <span className="tag tools-beta">Beta</span>}
                </span>
                <span className="tools-card-blurb">{t.blurb}</span>
              </button>
            ))}
          </div>
        </section>
      ))}
    </>
  );
}
