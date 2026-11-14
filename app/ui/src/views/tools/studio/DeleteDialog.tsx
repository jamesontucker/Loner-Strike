// The delete question for something the graph uses (a variable, a local, a Function, a Custom
// Event): how many nodes use it and where, Find uses, and what to do with them: Replace with…
// another that fits, Remove it and its nodes (the flow joined up round them), or Remove and keep
// the nodes (they show a problem until changed). Whatever is picked is one undo step.

import { useEffect, useRef, useState } from "react";
import { create } from "zustand";
import { Sheet } from "../../../ui";
import { useStudio } from "./store";
import { refName, removeKeepNodes, removeWithNodes, replaceRefs, replacements, usesOfRef, type DeleteRef } from "./deleteOps";
import { whereText } from "./parts";

type Ask = { ref: DeleteRef; done: (deleted: boolean) => void };
const useAsk = create<{ ask: Ask | null; set: (a: Ask | null) => void }>((set) => ({ ask: null, set: (ask) => set({ ask }) }));

const NOUN = { var: "variable", fn: "Function", cevent: "Custom Event" } as const;

/** Asks how to delete it (nothing uses it: deleted at once). Resolves true when it was deleted. */
export function askDelete(ref: DeleteRef): Promise<boolean> {
  const g = useStudio.getState().graph;
  if (!g) return Promise.resolve(false);
  if (!usesOfRef(g, ref).length) {
    useStudio.getState().edit((x) => removeKeepNodes(x, ref));
    return Promise.resolve(true);
  }
  return new Promise((done) => {
    useAsk.getState().ask?.done(false);
    useAsk.getState().set({ ref, done });
  });
}

type Choice = "replace" | "nodes" | "keep";

/** Mounted once on the page. `findUses` shows the uses on the canvas. */
export function DeleteDialogHost({ findUses }: { findUses: (ids: string[]) => void }) {
  const ask = useAsk((s) => s.ask);
  const graph = useStudio((s) => s.graph);
  const [choice, setChoice] = useState<Choice>("nodes");
  const [to, setTo] = useState("");
  const first = useRef<HTMLInputElement>(null);
  const ref = ask?.ref;
  const others = ref && graph ? replacements(graph, ref) : [];
  useEffect(() => {
    if (!ask) return;
    setChoice("nodes");
    setTo(others[0]?.id ?? "");
    setTimeout(() => first.current?.focus(), 0);
  }, [ask]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!ask || !ref || !graph) return null;
  const uses = usesOfRef(graph, ref);
  const name = refName(graph, ref);
  const noun = NOUN[ref.kind];
  const close = (deleted: boolean) => {
    ask.done(deleted);
    useAsk.getState().set(null);
  };
  const go = () => {
    const pick = choice === "replace" && !to ? "nodes" : choice;
    useStudio.getState().edit((g) => (pick === "replace" ? replaceRefs(g, ref, to) : pick === "nodes" ? removeWithNodes(g, ref) : removeKeepNodes(g, ref)));
    close(true);
  };
  const what = ref.kind === "var" ? "Get, Set and Add nodes" : ref.kind === "fn" ? "calls" : "Call nodes, timers and its Custom Event nodes";
  return (
    <Sheet label={`Delete the ${noun}`} onClose={() => close(false)}>
      <form
        className="st-del"
        data-testid="studio-delete-dialog"
        onSubmit={(e) => {
          e.preventDefault();
          go();
        }}
      >
        <h3>
          Delete the {noun} “{name}”?
        </h3>
        <p>
          It's used by {whereText(graph, { nodes: uses, others: [] })}.{" "}
          <button
            type="button"
            className="st-link"
            onClick={() => {
              close(false);
              findUses(uses.map((n) => n.id));
            }}
          >
            Find uses
          </button>
        </p>
        <fieldset className="st-del-choices">
          <legend className="st-muted">What happens to them</legend>
          <label className={others.length ? "" : "st-del-off"}>
            <input type="radio" name="del" value="replace" checked={choice === "replace"} disabled={!others.length} onChange={() => setChoice("replace")} />
            <span>
              <strong>Replace with</strong>{" "}
              <select className="st-field" aria-label={`Replace with which ${noun}`} value={to} disabled={!others.length} onChange={(e) => (setTo(e.target.value), setChoice("replace"))}>
                {others.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.name}
                  </option>
                ))}
              </select>
              <span className="st-muted st-del-help">{others.length ? `Its ${what} switch to the one you pick, wires and all.` : `No other ${noun} fits (the same type and scope${ref.kind === "var" ? "" : ", the same pins"}).`}</span>
            </span>
          </label>
          <label>
            <input ref={first} type="radio" name="del" value="nodes" checked={choice === "nodes"} onChange={() => setChoice("nodes")} />
            <span>
              <strong>Remove it and its nodes</strong>
              <span className="st-muted st-del-help">Its {what} go too. The flow is joined up round them; inputs they fed show a problem to fix.</span>
            </span>
          </label>
          <label>
            <input type="radio" name="del" value="keep" checked={choice === "keep"} onChange={() => setChoice("keep")} />
            <span>
              <strong>Remove it, keep the nodes</strong>
              <span className="st-muted st-del-help">The nodes stay where they are and show a problem until you pick another.</span>
            </span>
          </label>
        </fieldset>
        <div className="row st-del-actions">
          <button type="button" className="btn" onClick={() => close(false)}>
            Cancel
          </button>
          <button type="submit" className="btn primary danger" data-testid="studio-delete-go">
            {choice === "replace" && to ? "Replace and delete" : "Delete"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}
