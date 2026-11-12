// The Tools section: a row of tools (views/tools/registry.tsx), each loaded the first time
// it's opened. The page itself loads lazily too, with three.js in its own chunk.

import { Suspense, useEffect } from "react";
import { ON_THIS_COMPUTER } from "../../api";
import { useNav } from "../../store";
import { useTools } from "../../stores/tools";
import type { MapInfo } from "../../types";
import { lazyPage, SectionHead, Segmented, Skeleton } from "../../ui";
import { TOOLS } from "./registry";

const PAGES = new Map(TOOLS.map((t) => [t.id, lazyPage(t.load)]));

export default function Tools({ maps }: { maps: MapInfo[] }) {
  const focus = useNav((s) => s.focus);
  const tool = useTools((s) => s.tool);
  const setTool = useTools((s) => s.setTool);
  const Page = PAGES.get(tool) ?? PAGES.get(TOOLS[0].id);
  const blurb = TOOLS.find((t) => t.id === tool)?.blurb;

  // opened on a map ("map:de_dust2") or a tool ("replay") from elsewhere in the app
  useEffect(() => {
    if (!focus) return;
    const [what, arg] = [focus.split(":")[0], focus.slice(focus.indexOf(":") + 1)];
    if (what === "map" && arg) {
      setTool("map");
      useTools.getState().openMap(arg);
    } else if (what === "replay") {
      setTool("replay");
      if (arg && arg !== "replay") useTools.getState().openReplay(arg);
    } else if (PAGES.has(what)) {
      setTool(what);
      useTools.setState({ arg: arg && arg !== what ? arg : null });
    }
    useNav.getState().clearFocus();
  }, [focus, setTool]);

  return (
    <>
      <SectionHead
        title="Tools"
        note={
          <>
            <span className="tag tools-beta">Beta</span> New in 1.0: look inside maps and watch the bots. More tools are on the way. {blurb}
          </>
        }
      >
        <Segmented values={TOOLS.map((t) => t.id).join(" ")} labels={TOOLS.map((t) => t.label).join("|")} current={tool} onPick={setTool} label="Tool" />
      </SectionHead>
      {!ON_THIS_COMPUTER ? (
        <div className="panel">
          <h3>On the computer only</h3>
          <p className="muted measure">Tools work on the computer running Loner-Strike.</p>
        </div>
      ) : Page ? (
        <Suspense fallback={<Skeleton rows={4} />}>
          <Page maps={maps} />
        </Suspense>
      ) : null}
    </>
  );
}
