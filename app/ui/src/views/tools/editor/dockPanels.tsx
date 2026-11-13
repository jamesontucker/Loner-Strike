// P5: every panel the editor can dock, float or pop out. P4 and others: add a panel here (one
// entry) instead of a side tab; it then shows up in View > Panels too.

import type { ReactNode } from "react";
import { useToolArg } from "../../../stores/tools";
import { ChecksPanel, Inspector, TextureBrowser, Visibility } from "./Panels";
import { OutlinePanel } from "./OutlinePanel";
import { LibraryPanel } from "./LibraryPanel";
import { CompilePanel } from "./CompilePanel";
import { ProjectPanel } from "./ProjectPanel";
import { HistoryPanel } from "./HistoryPanel";
import { useEditor } from "./store";

export type DockPanel = { id: string; title: string; render: () => ReactNode; label?: () => string; bad?: () => boolean };

function Library() {
  const arg = useToolArg();
  return <LibraryPanel start={arg === "components" ? "components" : arg === "prefabs" ? "prefabs" : undefined} />;
}

const checksCount = () => {
  const c = useEditor.getState().checks;
  const problems = c?.issues.filter((i) => i.level === "error").length ?? 0;
  const warnings = c?.issues.filter((i) => i.level === "warning").length ?? 0;
  return problems ? `Checks (${problems})` : warnings ? `Checks (${warnings})` : "Checks";
};

export const DOCK_PANELS: DockPanel[] = [
  { id: "inspect", title: "Inspect", render: () => <Inspector /> },
  { id: "outline", title: "Outline", render: () => <OutlinePanel extra={<Visibility />} /> },
  { id: "textures", title: "Textures", render: () => <TextureBrowser /> },
  { id: "library", title: "Library", render: () => <Library /> },
  { id: "checks", title: "Checks", render: () => <ChecksPanel />, label: checksCount, bad: () => (useEditor.getState().checks?.issues.some((i) => i.level === "error") ?? false) },
  { id: "compile", title: "Compile", render: () => <CompilePanel /> },
  { id: "project", title: "Project", render: () => <ProjectPanel /> },
  { id: "history", title: "History", render: () => <HistoryPanel /> },
];

export const panelById = (id: string) => DOCK_PANELS.find((p) => p.id === id);
