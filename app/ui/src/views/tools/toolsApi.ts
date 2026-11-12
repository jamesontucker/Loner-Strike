// Requests for the Tools page (plan 3.8). Every route is on this computer only and answers
// {error} JSON in plain words when something goes wrong.
//
// In development, before the Rust routes exist (or with ?mock in the address, or
// localStorage "ls.toolsMock" = "1"), a generated test room stands in for every map. The
// mock is a separate file loaded only in development, so it never ships.

import { useQuery } from "@tanstack/react-query";
import { postJson } from "../../api";
import { useActivityStore } from "../../store";
import { decodeKnow, decodeReplay, toNav, toTrace, type Know, type Nav, type RecordingInfo } from "./overlays";
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
  /** json: the app answered in its own words (not a missing route) */
  constructor(message: string, readonly status = 0, readonly json = false) {
    super(message);
  }
}

/** JSON routes answer {error} with 200 (like the rest of the app): that's a failure too. */
function checked<T>(v: unknown): T {
  if (v && typeof v === "object" && !Array.isArray(v) && typeof (v as { error?: unknown }).error === "string") throw new ToolsError((v as { error: string }).error, 200, true);
  return v as T;
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
const noServer = (e: ToolsError) => import.meta.env.DEV && !e.json && [0, 404, 500, 502, 504].includes(e.status);

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
  throw new ToolsError(msg || (r.status === 404 ? "That map isn't in your game any more." : "Something went wrong reading this map. Try again."), r.status, json && !!msg);
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
      if (e instanceof ToolsError && noServer(e)) {
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
  const r = checked<unknown>(await (await fetchOk(`${base(map)}/entities`, signal)).json());
  return Array.isArray(r) ? r.map(toEntity) : [];
}

async function getInfo(map: string, signal?: AbortSignal): Promise<ToolsMapInfo | null> {
  if (forcedMock() || mockOn) return (await loadMock()).mockInfo();
  return checked<ToolsMapInfo>(await (await fetchOk(`${base(map)}/info`, signal)).json());
}

/** A texture's picture (RGBA PNG); a failed load draws a checker. `mock`: the bundle is the dev test room. */
export function texUrl(map: string, name: string, mock = false): Promise<string | null> {
  if (mock) return loadMock().then((m) => m.mockTexture(name));
  return Promise.resolve(`${base(map)}/tex/${encodeURIComponent(name)}.png`);
}
/** The map's lighting as one picture; a failed load draws without lighting. */
export function lightmapUrl(map: string, mock = false): Promise<string | null> {
  if (mock) return loadMock().then((m) => m.mockLightmap());
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

// ---- the bots' data: nav, knowledge, recordings (agent C's routes) --------------------------

async function getNav(map: string, signal?: AbortSignal): Promise<Nav> {
  if (forcedMock() || mockOn) throw new ToolsError("The test room has no nav.", 200, true);
  const r = checked<Record<string, unknown>>(await (await fetchOk(`${base(map)}/nav`, signal)).json());
  return toNav(r);
}

async function getKnow(map: string, signal?: AbortSignal): Promise<Know> {
  if (forcedMock() || mockOn) throw new ToolsError("The test room has no knowledge.", 200, true);
  return decodeKnow(await (await fetchOk(`${base(map)}/know`, signal)).arrayBuffer());
}

/** The map's nav (only fetched while a nav layer is on). */
export function useNav(map: string | null, on: boolean) {
  return useQuery({ queryKey: ["tools", "nav", map], queryFn: ({ signal }) => getNav(map!, signal), enabled: !!map && on, retry: false, staleTime: 30_000 });
}
/** What the bots learned about the map (.lsv). */
export function useKnow(map: string | null, on: boolean) {
  return useQuery({ queryKey: ["tools", "know", map], queryFn: ({ signal }) => getKnow(map!, signal), enabled: !!map && on, retry: false, staleTime: 30_000, structuralSharing: false });
}

export function useRecordings(on: boolean) {
  return useQuery({
    queryKey: ["tools", "recordings"],
    queryFn: async ({ signal }) => {
      const r = checked<{ recordings?: RecordingInfo[]; folders?: string[] }>(await (await fetchOk("/api/tools/recordings", signal)).json());
      return { recordings: Array.isArray(r.recordings) ? r.recordings : [], folders: Array.isArray(r.folders) ? r.folders : [] };
    },
    enabled: on,
    retry: false,
  });
}

/** Asks for a recording file (the app's file dialog). null when cancelled. */
export async function openRecording(): Promise<{ id: string; recording?: RecordingInfo } | null> {
  const r = await postJson<{ id?: string; recording?: RecordingInfo; cancelled?: boolean; error?: string }>("/api/tools/recordings/open");
  if (r.error) throw new ToolsError(r.error, 200, true);
  if (r.cancelled || !r.id) return null;
  return { id: r.id, recording: r.recording };
}

export function useReplay(id: string | null) {
  return useQuery({
    queryKey: ["tools", "recording", id],
    queryFn: ({ signal }) =>
      withActivity(`tools:replay:${id}`, "Reading the recording…", async () => decodeReplay(await (await fetchOk(`/api/tools/recording/${encodeURIComponent(id!)}`, signal)).arrayBuffer())),
    enabled: !!id,
    retry: false,
    staleTime: Infinity,
    gcTime: 60_000,
    structuralSharing: false,
  });
}

export function useTrace(id: string | null, on: boolean) {
  return useQuery({
    queryKey: ["tools", "trace", id],
    queryFn: async ({ signal }) => toTrace(checked<Record<string, unknown>>(await (await fetchOk(`/api/tools/recording/${encodeURIComponent(id!)}/trace`, signal)).json())),
    enabled: !!id && on,
    retry: false,
    staleTime: Infinity,
    structuralSharing: false,
  });
}
