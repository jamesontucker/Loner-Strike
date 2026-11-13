import { create } from "zustand";

// Which tool the Tools section shows ("" is the overview of cards; else an id from
// views/tools/registry.tsx). Its own tiny store so the sidebar can highlight the open tool
// without loading the Tools page's view state (stores/tools.ts) at start.
export const useToolPage = create<{ tool: string }>(() => ({ tool: "" }));
