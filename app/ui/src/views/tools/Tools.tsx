// The Tools section: an overview of cards, or one tool (views/tools/registry.tsx), each
// loaded the first time it's opened. The sidebar lists the tools under Tools; every tool has
// an address: go("tools", "<id>") or go("tools", "<id>:<arg>"), go("tools", "overview") for
// the cards. The page itself loads lazily too, with three.js in its own chunk.

import { Suspense, useEffect } from "react";
import { ON_THIS_COMPUTER } from "../../api";
import { useNav } from "../../store";
import { useToolPage } from "../../stores/toolPage";
import type { MapInfo } from "../../types";
import { lazyPage, SectionHead, Skeleton } from "../../ui";
import { TOOL_GROUPS, TOOLS } from "./registry";
import { lastTool, rememberWhereYouAre } from "./common/address";
import { Breadcrumbs, FocusButton, FocusPill, openAddress, useShell, useToolsShell } from "./common/shell";
import { ToolKeys } from "./common/Shortcuts";
import { ToolHistory } from "./common/History";
import "./tools.css";
import "./common/common.css";

const PAGES = new Map(TOOLS.map((t) => [t.id, lazyPage(t.load)]));

export default function Tools({ maps }: { maps: MapInfo[] }) {
  const focus = useNav((s) => s.focus);
  const tool = useToolPage((s) => s.tool);
  const entry = TOOLS.find((t) => t.id === tool);
  const Page = entry && PAGES.get(entry.id);
  // bumped to start the page over (a crumb, Back)
  const n = useShell((s) => s.n);

  // Back landing in Tools opens where it was; with nothing asked for, where you were last time
  useToolsShell(() => {
    if (!useNav.getState().focus && !useToolPage.getState().tool && lastTool()) openAddress(lastTool());
  });
  useEffect(() => rememberWhereYouAre(), []);

  // opened on a tool from the sidebar or elsewhere in the app
  useEffect(() => {
    if (!focus) return;
    openAddress(focus);
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
  if (!entry || !Page)
    return (
      <>
        <Overview />
        <ToolKeys />
      </>
    );
  return (
    <>
      {/* (focus mode hides what the app shows above this) */}
      <span className="tools-start" hidden />
      <div className="section-head tools-head">
        <Breadcrumbs />
        <div className="section-title">
          <h2>{entry.label}</h2>
          <div className="section-actions">
            <ToolHistory tool={entry.id} />
            <FocusButton />
          </div>
        </div>
        <p>
          {entry.beta && <span className="tag tools-beta">Beta</span>} {entry.blurb}
        </p>
      </div>
      <Suspense fallback={<Skeleton rows={4} />}>
        <Page key={`${entry.id}:${n}`} maps={maps} />
      </Suspense>
      <ToolKeys />
      <FocusPill />
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
