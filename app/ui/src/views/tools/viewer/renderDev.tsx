// Development only: the viewer alone, filling the window, for comparing it with the game
// (render-dev.html, docs/TOOLS-RENDER.md). ?map=de_dust2&look=game|bright&cam=x,y,z,pitch,yaw
// puts the camera where `setpos x y z; setang pitch yaw 0` puts the player's.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { createRoot } from "react-dom/client";
import { useTools } from "../../../stores/tools";
import type { Overlays } from "../MapViewer";
import { useEntities, useMesh } from "../toolsApi";
import { useLookMode } from "./look";
import { Scene } from "./Scene";
import { useModelMeshes } from "./World";

const q = new URLSearchParams(location.search);
const map = q.get("map") ?? "de_dust2";
const none: Overlays = { nav: null, navError: null, navLoading: false, know: null, knowError: null, knowLoading: false, replay: null, trace: null };

function View() {
  const mesh = useMesh(map);
  const ents = useEntities(map, !!mesh.data);
  const meshes = useModelMeshes(mesh.data ?? null, ents.data ?? []);
  useEffect(() => {
    useLookMode.getState().setMode(q.get("look") === "bright" ? "bright" : "game");
    const s = useTools.getState();
    s.setCamera("fly");
    for (const k of ["entities", "triggers"] as const) if (s.layers[k]) s.setLayer(k, false);
    for (const k of ["textures", "lighting", "moving"] as const) if (!s.layers[k]) s.setLayer(k, true);
  }, []);
  const ready = !!mesh.data && !!ents.data;
  useEffect(() => {
    const cam = q.get("cam")?.split(",").map(Number);
    if (!ready || !cam || cam.length < 5 || !cam.every(Number.isFinite)) return;
    const t = setInterval(() => {
      if (!window.__lsViewer) return;
      clearInterval(t);
      window.__lsViewer.camera(cam[0], cam[1], cam[2], cam[3], cam[4], cam[5] ?? 90);
      document.body.dataset.ready = "1";
    }, 100);
    return () => clearInterval(t);
  }, [ready]);
  if (mesh.error) return <p style={{ color: "#fff" }}>{String(mesh.error)}</p>;
  if (!ready) return <p style={{ color: "#fff" }}>Reading {map}…</p>;
  return <Scene bundle={mesh.data!} meshes={meshes} entities={ents.data!} map={map} overlays={none} onLost={() => location.reload()} />;
}

createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={new QueryClient()}>
    <View />
  </QueryClientProvider>,
);
