// The Tools section: an overview of cards, or one tool (views/tools/registry.tsx), each
// loaded the first time it's opened. The sidebar lists the tools under Tools; every tool has
// an address: go("tools", "<id>") or go("tools", "<id>:<arg>"), go("tools", "overview") for
// the cards. The page itself loads lazily too, with three.js in its own chunk.

import { DocsLink, ReferenceLink, TOOL_DOCS, TOOL_REFERENCE } from "../DocsLink"; // P5
import { Suspense, useEffect } from "react";
import { ON_THIS_COMPUTER } from "../../api";
import { useNav } from "../../store";
import { useToolPage } from "../../stores/toolPage";
import type { MapInfo } from "../../types";
import { Icon } from "../../icons";
import { lazyPage, SectionHead, Skeleton } from "../../ui";
import { TOOL_GROUPS, TOOL_GROUP_BLURBS, TOOLS } from "./registry";
import { lastTool, rememberWhereYouAre, useReportedAddress } from "./common/address";
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
  // the docs section for the header's Docs button (Community's Publish has its own)
  const where = useReportedAddress(entry?.id ?? "");
  const docs = entry && (entry.id === "community" && where?.startsWith("community:publish") ? "publishing" : TOOL_DOCS[entry.id]);

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
            {docs && <DocsLink to={docs} what={docs === "publishing" ? "publishing" : entry.label} />}
            {TOOL_REFERENCE[entry.id] && <ReferenceLink page={TOOL_REFERENCE[entry.id].page} what={TOOL_REFERENCE[entry.id].what} />}
            <FocusButton />
          </div>
        </div>
        <p>
          {entry.beta && <span className="tag tools-beta">Beta</span>} {entry.blurb}
        </p>
      </div>
      <FocusPill label={entry.label} />
      <Suspense fallback={<Skeleton rows={4} />}>
        <Page key={`${entry.id}:${n}`} maps={maps} />
      </Suspense>
      <ToolKeys />
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
            <span className="tag tools-beta">Beta</span> New in 1.0: make your own maps and modes, get any map ready for bots, and make the game yours.
          </>
        }
      />
      {TOOL_GROUPS.map((g) => (
        <section key={g} className="tools-cards-group">
          <h3 className="tools-cards-head">{g}</h3>
          <p className="tools-cards-blurb">{TOOL_GROUP_BLURBS[g]}</p>
          <div className="tools-cards">
            {TOOLS.filter((t) => t.group === g).map((t) => (
              <button key={t.id} className="tools-card" title={t.blurb} onClick={() => go("tools", t.id)}>
                <span className="tools-card-icon">
                  <Icon name={t.icon ?? "tools"} size={22} />
                </span>
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
