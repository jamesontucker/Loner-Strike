// Requests for the Tools page (plan 3.8). Every route is on this computer only and answers
// {error} JSON in plain words when something goes wrong.
//
// In development, before the Rust routes exist (or with ?mock in the address, or
// localStorage "ls.toolsMock" = "1"), a generated test room stands in for every map. The
// mock is a separate file loaded only in development, so it never ships.

import { useQuery } from "@tanstack/react-query";
import { useActivityStore } from "../../store";
import { decodeBundle, type MeshBundle } from "./viewer/bundle";
import { parseVec, type Vec3 } from "./viewer/coords";

export type MapEntity = {
  index: number;
  classname: string;
  origin: Vec3 | null;
  angles: Vec3 | null;
  /** "*N" for a brush entity, or a model file */
  model: string | null;
  keys: Record<string, string>;
};

export type ToolsMapInfo = {
  bounds?: { min: Vec3; max: Vec3 };
  textures?: { name: string; from: string }[];
  skyname?: string;
  message?: string;
  [k: string]: unknown;
};

/** A request the app answered with an error (its own words when it gave some). */
export class ToolsError extends Error {
  constructor(message: string, readonly status = 0) {
    super(message);
  }
}

const base = (map: string) => `/api/tools/map/${encodeURIComponent(map)}`;

// ---- the dev mock --------------------------------------------------------------------------

type Mock = typeof import("./mock");
let mock: Promise<Mock> | null = null;
let mockOn = false;
function forcedMock() {
  if (!import.meta.env.DEV) return false;
  try {
    return new URLSearchParams(location.search).has("mock") || localStorage.getItem("ls.toolsMock") === "1";
  } catch {
    return false;
  }
}
// (import.meta.env.DEV is false in a release build, so the mock file isn't even built)
const loadMock = () => (mock ??= import.meta.env.DEV ? import("./mock") : Promise.reject(new ToolsError("Not available.")));
/** In development: whether a failed answer means the Rust routes aren't there yet. */
const noServer = (status: number, json: boolean) => import.meta.env.DEV && (status === 404 || ((status === 500 || status === 502 || status === 504) && !json));

// ---- requests ------------------------------------------------------------------------------

async function fetchOk(path: string, signal?: AbortSignal): Promise<Response> {
  let r: Response;
  try {
    r = await fetch(path, { credentials: "same-origin", signal });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    throw new ToolsError("Couldn't reach the app. Try again.");
  }
  if (r.ok) return r;
  const json = (r.headers.get("content-type") ?? "").includes("json");
  let msg = "";
  if (json) {
    try {
      msg = String((await r.json()).error ?? "");
    } catch {
      /* no message */
    }
  }
  throw new ToolsError(msg || (r.status === 404 ? "That map isn't in your game any more." : "Something went wrong reading this map. Try again."), r.status);
}

/** Runs a slow read with the app's spinner and label in the top bar (kept across pages). */
async function withActivity<T>(key: string, label: string, fn: () => Promise<T>): Promise<T> {
  const { start, end } = useActivityStore.getState();
  const mine = start(key, label);
  try {
    return await fn();
  } finally {
    if (mine) end(key);
  }
}

async function getMesh(map: string, signal?: AbortSignal): Promise<MeshBundle> {
  return withActivity(`tools:open:${map}`, "Reading the map…", async () => {
    if (forcedMock() || mockOn) return (await loadMock()).mockBundle();
    let buf: ArrayBuffer;
    try {
      buf = await (await fetchOk(`${base(map)}/mesh`, signal)).arrayBuffer();
    } catch (e) {
      if (e instanceof ToolsError && (noServer(e.status, false) || (import.meta.env.DEV && e.status === 0))) {
        console.info("Tools: the app's map routes aren't answering, so a test room stands in (dev only).");
        mockOn = true;
        return (await loadMock()).mockBundle();
      }
      throw e;
    }
    return decodeBundle(buf);
  });
}

function toEntity(e: Record<string, unknown>, i: number): MapEntity {
  const keys: Record<string, string> = {};
  const raw = e.keys && typeof e.keys === "object" ? (e.keys as Record<string, unknown>) : {};
  for (const [k, v] of Object.entries(raw)) keys[k] = String(v);
  const classname = String(e.classname ?? keys.classname ?? "");
  return {
    index: typeof e.index === "number" ? e.index : i,
    classname,
    origin: parseVec(e.origin ?? keys.origin),
    angles: parseVec(e.angles ?? keys.angles),
    model: (e.model ?? keys.model ?? null) as string | null,
    keys,
  };
}

async function getEntities(map: string, signal?: AbortSignal): Promise<MapEntity[]> {
  if (forcedMock() || mockOn) return (await loadMock()).mockEntities().map(toEntity);
  const r = await (await fetchOk(`${base(map)}/entities`, signal)).json();
  return Array.isArray(r) ? r.map(toEntity) : [];
}

async function getInfo(map: string, signal?: AbortSignal): Promise<ToolsMapInfo | null> {
  if (forcedMock() || mockOn) return (await loadMock()).mockInfo();
  return (await (await fetchOk(`${base(map)}/info`, signal)).json()) as ToolsMapInfo;
}

/** A texture's picture (RGBA PNG); a failed load draws a checker. */
export function texUrl(map: string, name: string): Promise<string | null> {
  if (forcedMock() || mockOn) return loadMock().then((m) => m.mockTexture(name));
  return Promise.resolve(`${base(map)}/tex/${encodeURIComponent(name)}.png`);
}
/** The map's lighting as one picture; a failed load draws without lighting. */
export function lightmapUrl(map: string): Promise<string | null> {
  if (forcedMock() || mockOn) return loadMock().then((m) => m.mockLightmap());
  return Promise.resolve(`${base(map)}/lightmap.png`);
}

// ---- hooks -------------------------------------------------------------------------------

export function useMesh(map: string | null) {
  return useQuery({
    queryKey: ["tools", "mesh", map],
    queryFn: ({ signal }) => getMesh(map!, signal),
    enabled: !!map,
    // typed arrays: nothing to compare, and big
    structuralSharing: false,
    retry: false,
    staleTime: Infinity,
    gcTime: 60_000,
  });
}

export function useEntities(map: string | null, ready = true) {
  return useQuery({
    queryKey: ["tools", "entities", map],
    queryFn: ({ signal }) => getEntities(map!, signal),
    enabled: !!map && ready,
    retry: false,
    staleTime: Infinity,
  });
}

export function useToolsInfo(map: string | null, ready = true) {
  return useQuery({
    queryKey: ["tools", "info", map],
    queryFn: ({ signal }) => getInfo(map!, signal).catch(() => null),
    enabled: !!map && ready,
    retry: false,
    staleTime: Infinity,
  });
}
