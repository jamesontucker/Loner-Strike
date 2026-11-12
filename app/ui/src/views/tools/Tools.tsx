// The Tools section: tools for looking inside maps (and, later, fixing them and watching
// the bots). Loaded only when it's opened, with three.js in its own chunks.

import { useEffect } from "react";
import { ON_THIS_COMPUTER } from "../../api";
import { useNav } from "../../store";
import { useTools, type Tool } from "../../stores/tools";
import type { MapInfo } from "../../types";
import { SectionHead, Segmented } from "../../ui";
import { MapViewer } from "./MapViewer";

export default function Tools({ maps }: { maps: MapInfo[] }) {
  const focus = useNav((s) => s.focus);
  const tool = useTools((s) => s.tool);
  const setTool = useTools((s) => s.setTool);

  // opened on a map ("map:de_dust2") or a tool ("replay") from elsewhere in the app
  useEffect(() => {
    if (!focus) return;
    const [what, arg] = [focus.split(":")[0], focus.slice(focus.indexOf(":") + 1)];
    if (what === "map" && arg) {
      setTool("map");
      useTools.getState().openMap(arg);
    } else if (what === "replay") setTool("replay");
    useNav.getState().clearFocus();
  }, [focus, setTool]);

  return (
    <>
      <SectionHead
        title="Tools"
        note={
          <>
            <span className="tag tools-beta">Beta</span> New in 1.0: look inside maps and watch the bots. More tools are on the way. Walk around a map, see every spawn, door and trigger, and click anything to see what it is.
          </>
        }
      >
        <Segmented values="map replay" labels="Map viewer|Replay" current={tool} onPick={(v) => setTool(v as Tool)} label="Tool" />
      </SectionHead>
      {!ON_THIS_COMPUTER ? (
        <div className="panel">
          <h3>On the computer only</h3>
          <p className="muted measure">Tools work on the computer running Loner-Strike.</p>
        </div>
      ) : tool === "map" ? (
        <MapViewer maps={maps} />
      ) : (
        <div className="panel">
          <h3>Replay is coming</h3>
          <p className="muted measure">Play back a recorded bot match on the map: where every bot went, what it saw, and where it fell. It's next on the list.</p>
        </div>
      )}
    </>
  );
}
