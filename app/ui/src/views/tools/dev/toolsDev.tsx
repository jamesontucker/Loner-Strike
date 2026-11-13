// Development only: the Tools section alone, in the app's layout, against the Rust routes
// served by `npm run tools:dev` (docs/TOOLS-DEV.md). Not part of the app's build.
//
// ?open=<address> opens a tool, the same addresses the app uses: "map:de_dust2@0,0,64",
// "navfix:cs_rats2", "editor", "overview". The address bar follows what's open, so a reload
// (or a Playwright test) lands on the same page. ?w=<px> narrows the content for layout checks.

import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { Suspense, useEffect } from "react";
import { createRoot } from "react-dom/client";
import "../../../styles.css";
import { useNav } from "../../../store";
import { useToolPage } from "../../../stores/toolPage";
import type { MapInfo } from "../../../types";
import { ConfirmProvider, PageBoundary, Skeleton, ToastProvider } from "../../../ui";
import { currentAddress, subscribeAddress } from "../common/address";
import { get } from "../common/api";
import { TOOL_GROUPS, TOOLS } from "../registry";
import Tools from "../Tools";

const q = new URLSearchParams(location.search);

/** /api/tools/maps facts, shaped like the app's map list. */
function useDevMaps(): MapInfo[] {
  const r = useQuery({
    queryKey: ["tools-dev", "maps"],
    queryFn: () => get<{ maps: Record<string, unknown>[] }>("/api/tools/maps"),
    staleTime: Infinity,
  });
  return (r.data?.maps ?? []).map((m) => ({
    name: String(m.name),
    title: typeof m.title === "string" ? m.title : undefined,
    cat: typeof m.cat === "string" ? m.cat : "",
    nav: !!m.nav,
    thumb: null,
    by: "",
    source: m.source === "download" || m.source === "added" ? m.source : "stock",
    fav: false,
    hidden: false,
    recent: 0,
    other: !!m.other,
  }));
}

function Shell() {
  const maps = useDevMaps();
  const tool = useToolPage((s) => s.tool);
  const go = useNav((s) => s.go);

  useEffect(() => {
    go("tools", q.get("open") || "overview");
    // the address bar follows what's open
    return subscribeAddress(() => {
      const u = new URL(location.href);
      u.searchParams.set("open", currentAddress() || "overview");
      history.replaceState(null, "", u);
    });
  }, [go]);

  const width = Number(q.get("w")) || 0;
  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">Tools (dev)</div>
        <nav className="nav" aria-label="Tools">
          <button className="nav-item" aria-current={!tool ? "page" : undefined} onClick={() => go("tools", "overview")}>
            Overview
          </button>
          {TOOL_GROUPS.map((g) => (
            <div key={g}>
              <div className="nav-group">{g}</div>
              {TOOLS.filter((t) => t.group === g).map((t) => (
                <button key={t.id} className="nav-item" aria-current={tool === t.id ? "page" : undefined} onClick={() => go("tools", t.id)}>
                  {t.label}
                </button>
              ))}
            </div>
          ))}
        </nav>
      </aside>
      <div className="main">
        <main className="content" style={width ? { maxWidth: width } : undefined}>
          <PageBoundary>
            <Suspense fallback={<Skeleton rows={4} />}>
              <Tools maps={maps} />
            </Suspense>
          </PageBoundary>
        </main>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } } })}>
    <ToastProvider>
      <ConfirmProvider>
        <Shell />
      </ConfirmProvider>
    </ToastProvider>
  </QueryClientProvider>,
);
