// The Plugin Studio's editor: Monaco with Pawn, and completions, hover docs and signature
// help from the include files' word list. Loaded on demand (import("./monaco")), so Monaco
// sits in its own chunk and the app's start bundle doesn't change. Only the core editor is
// bundled (no TypeScript/JSON/CSS languages): one plain editor worker.

import "monaco-editor/esm/vs/editor/edcore.main";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { KEYWORDS, pawnConfig, pawnGrammar, pawnTheme } from "./pawn";
import { docsUrl } from "./docs";
import type { Diag, Word, Words } from "./pluginsApi";
import { SNIPPETS, TEMPLATES } from "./templates";

(self as unknown as { MonacoEnvironment: monaco.Environment }).MonacoEnvironment = { getWorker: () => new EditorWorker() };

// Monaco cancels its own pending work (a hover, a suggestion) when a file is switched or closed,
// and that cancel arrives as an unhandled "Canceled" rejection: not an error.
window.addEventListener("unhandledrejection", (e) => {
  const r = e.reason as { name?: string; message?: string } | undefined;
  if (r && (r.name === "Canceled" || r.message === "Canceled")) e.preventDefault();
});

const LANG = "pawn";
let words: Words = { includes: [], symbols: [] };
let byName = new Map<string, Word>();
let ready = false;

const FUNCS = new Set(["native", "stock", "forward", "macro"]);

/** What the page does for the editor: open an include at a line (go to definition), read an
 *  include's text (peek), and the "/" menu's commands. Set by createEditor. */
export type PageHooks = {
  openInclude: (file: string, line: number) => void;
  readInclude: (file: string) => Promise<string | null>;
  newPlugin: () => void;
  docs: (query: string) => void;
};
let hooks: PageHooks | null = null;
/** the next suggest is the "/" menu (the Insert button) */
let paletteNext = false;
/** the editor's own command ids for the "/" menu's commands */
let cmdIds: { newPlugin?: string; docs?: string } = {};

function setup() {
  if (ready) return;
  ready = true;
  monaco.languages.register({ id: LANG, extensions: [".sma", ".inc"], aliases: ["Pawn", "AMX Mod X"] });
  monaco.languages.setLanguageConfiguration(LANG, pawnConfig);
  monaco.languages.setMonarchTokensProvider(LANG, pawnGrammar([], []));
  monaco.editor.defineTheme("ls-pawn", pawnTheme);
  monaco.languages.registerCompletionItemProvider(LANG, { triggerCharacters: ["<", "#", "/"], provideCompletionItems: complete });
  monaco.languages.registerHoverProvider(LANG, { provideHover: hover });
  monaco.languages.registerSignatureHelpProvider(LANG, { signatureHelpTriggerCharacters: ["(", ","], signatureHelpRetriggerCharacters: [","], provideSignatureHelp: signature });
  monaco.languages.registerDefinitionProvider(LANG, { provideDefinition: definition });
  // go to definition into an include: the page opens it (read only) at that line
  monaco.editor.registerEditorOpener({
    openCodeEditor(_source, resource, at) {
      const m = /^\/include\/(.+)$/.exec(decodeURIComponent(resource.path));
      if (!m || resource.scheme !== "inmemory" || !hooks) return false;
      const line = at && "lineNumber" in at ? at.lineNumber : at && "startLineNumber" in at ? at.startLineNumber : 1;
      hooks.openInclude(m[1], line);
      return true;
    },
  });
  // links in the docs (AMX Mod X's API pages) open in the browser
  monaco.editor.registerLinkOpener({
    open(uri) {
      if (uri.scheme !== "https") return false;
      window.open(uri.toString(true), "_blank", "noopener,noreferrer");
      return true;
    },
  });
}

/** The include files' words arrived: completions and colours use them from now on. */
export function setWords(w: Words) {
  setup();
  words = w;
  byName = new Map(w.symbols.map((s) => [s.name, s]));
  const fns = w.symbols.filter((s) => FUNCS.has(s.kind)).map((s) => s.name);
  const consts = w.symbols.filter((s) => s.kind === "constant" || s.kind === "define").map((s) => s.name);
  monaco.languages.setMonarchTokensProvider(LANG, pawnGrammar(fns, consts));
}

// ---- docs as markdown ----------------------------------------------------------------

const KIND_WORD: Record<Word["kind"], string> = { native: "native", forward: "forward (your public function the game calls)", stock: "stock", define: "#define", macro: "macro", constant: "constant", enum: "enum" };

function docOf(s: Word): monaco.IMarkdownString {
  const parts: string[] = ["```pawn\n" + s.detail + "\n```"];
  if (s.deprecated !== undefined) parts.push(`**Deprecated.** ${s.deprecated}`);
  if (s.doc) parts.push(s.doc);
  const params = s.params.filter((p) => p.doc);
  if (params.length) parts.push(params.map((p) => `- \`${p.name}\`: ${p.doc}`).join("\n"));
  if (s.returns) parts.push(`**Returns** ${s.returns}`);
  for (const n of s.notes ?? []) parts.push(`*Note:* ${n}`);
  if (s.error) parts.push(`*Errors:* ${s.error}`);
  const url = docsUrl(s);
  parts.push(`<small>${KIND_WORD[s.kind]} · from ${s.file}${s.kind === "enum" || FUNCS.has(s.kind) || s.kind === "define" || s.kind === "constant" ? " · F12 opens it there" : ""}</small>`);
  if (url) parts.push(`[Open in the AMX Mod X docs](${url})`);
  return { value: parts.join("\n\n"), supportHtml: true, isTrusted: false };
}

// ---- the current file's own names ----------------------------------------------------

type Local = { name: string; kind: "function" | "variable" | "define"; line: number; text: string };

function locals(model: monaco.editor.ITextModel): Local[] {
  const out: Local[] = [];
  const seen = new Set<string>();
  const lines = model.getLinesContent();
  lines.forEach((text, i) => {
    let m: RegExpMatchArray | null;
    const add = (name: string, kind: Local["kind"]) => {
      if (!seen.has(name) && !KEYWORDS.includes(name)) {
        seen.add(name);
        out.push({ name, kind, line: i + 1, text: text.trim() });
      }
    };
    if ((m = text.match(/^\s*#define\s+([A-Za-z_]\w*)/))) add(m[1], "define");
    // a function: "public name(", "stock Float:name(", or "name(" at the line's start (a
    // call is indented: Pawn's ; is optional, so a call line looks like a header)
    else if ((m = text.match(/^(?:\s*(?:public|stock|static)\s+|(?=[A-Za-z_@]))(?:[A-Za-z_]\w*:)?([A-Za-z_@][\w@]*)\s*\([^;]*$/)) && !/^\s*(if|for|while|switch|return|else)\b/.test(text) && !byName.has(m[1])) add(m[1], "function");
    for (const v of text.matchAll(/\b(?:new|static)\s+((?:[A-Za-z_]\w*:)?[A-Za-z_]\w*(?:\[[^\]]*\])*(?:\s*=\s*[^,;]+)?(?:\s*,\s*(?:[A-Za-z_]\w*:)?[A-Za-z_]\w*(?:\[[^\]]*\])*(?:\s*=\s*[^,;]+)?)*)/g)) {
      for (const part of v[1].split(",")) {
        const nm = part.trim().replace(/^[A-Za-z_]\w*:/, "").match(/^[A-Za-z_]\w*/);
        if (nm) add(nm[0], "variable");
      }
    }
  });
  return out;
}

// ---- completion ----------------------------------------------------------------------

const K = monaco.languages.CompletionItemKind;

/** "name(${1:a}, ${2:b})" with the parameters that have no default. */
function callSnippet(s: Word) {
  const need = s.params.filter((p) => !p.label.includes("=") && p.name !== "...");
  return `${s.name}(${need.map((p, i) => `\${${i + 1}:${p.name}}`).join(", ")})`;
}

function inCommentOrString(before: string) {
  const code = before.replace(/"(?:[^"^]|\^.)*"/g, '""');
  return code.includes("//") || (code.match(/"/g)?.length ?? 0) % 2 === 1;
}

function complete(model: monaco.editor.ITextModel, pos: monaco.Position): monaco.languages.CompletionList {
  const line = model.getLineContent(pos.lineNumber);
  const before = line.slice(0, pos.column - 1);
  const w = model.getWordUntilPosition(pos);
  const range = new monaco.Range(pos.lineNumber, w.startColumn, pos.lineNumber, w.endColumn);

  // #include <...>: the include files
  const inc = before.match(/^\s*#\s*(?:try)?include\s*[<"]([\w]*)$/);
  if (inc) {
    const r = new monaco.Range(pos.lineNumber, pos.column - inc[1].length, pos.lineNumber, pos.column);
    return { suggestions: [...new Set([...words.includes, "loner"])].map((n) => ({ label: n, kind: K.File, insertText: n, range: r, detail: `${n}.inc` })) };
  }
  if (/^\s*#\s*\w*$/.test(before)) {
    const r = new monaco.Range(pos.lineNumber, before.indexOf("#") + 1, pos.lineNumber, pos.column);
    return { suggestions: ["include", "define", "pragma", "if", "else", "endif", "tryinclude", "undef"].map((d) => ({ label: `#${d}`, kind: K.Keyword, insertText: `#${d} `, range: r })) };
  }
  // the "/" menu: "/" at the start of a line (or the Insert button)
  const slash = before.match(/^(\s*)\/(\w*)$/);
  if (paletteNext || slash) {
    const usePalette = paletteNext;
    paletteNext = false;
    const from = slash && !usePalette ? slash[1].length + 1 : pos.column;
    return { suggestions: palette(model, new monaco.Range(pos.lineNumber, from, pos.lineNumber, pos.column), !usePalette) };
  }
  if (inCommentOrString(before)) return { suggestions: [] };

  const atLineStart = /^\s*(public\s+)?\w*$/.test(before);
  const afterPublic = /^\s*public\s+\w*$/.test(before);
  const out: monaco.languages.CompletionItem[] = [];
  const snippet = monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet;

  for (const s of words.symbols) {
    const doc = docOf(s);
    if (s.kind === "forward") {
      if (!atLineStart) continue;
      const params = s.params.map((p) => p.label).join(", ");
      out.push({
        label: { label: s.name, description: "forward" },
        kind: K.Event,
        detail: s.detail,
        documentation: doc,
        insertText: `${afterPublic ? "" : "public "}${s.name}(${params})\n{\n\t$0\n}`,
        insertTextRules: snippet,
        range,
        sortText: `1${s.name}`,
      });
    } else if (s.kind === "native" || s.kind === "stock" || s.kind === "macro") {
      if (afterPublic) continue;
      out.push({
        label: { label: s.name, description: s.file.replace(/\.inc$/, "") },
        kind: s.kind === "macro" ? K.Snippet : K.Function,
        detail: s.detail,
        documentation: doc,
        insertText: s.kind === "macro" ? `${s.name}(\${1})` : callSnippet(s),
        insertTextRules: snippet,
        range,
        sortText: `${s.file === "loner.inc" ? "2" : "3"}${s.name}`,
        tags: s.deprecated !== undefined ? [monaco.languages.CompletionItemTag.Deprecated] : undefined,
        command: { id: "editor.action.triggerParameterHints", title: "" },
      });
    } else if (!afterPublic) {
      out.push({
        label: { label: s.name, description: s.value ?? "" },
        kind: s.kind === "enum" ? K.Enum : K.Constant,
        detail: s.detail,
        documentation: doc,
        insertText: s.name,
        range,
        sortText: `4${s.name}`,
      });
    }
  }
  if (!afterPublic) {
    for (const l of locals(model)) {
      if (l.line === pos.lineNumber) continue;
      out.push({ label: l.name, kind: l.kind === "function" ? K.Method : l.kind === "define" ? K.Constant : K.Variable, detail: l.text, insertText: l.name, range, sortText: `0${l.name}` });
    }
    for (const k of KEYWORDS) out.push({ label: k, kind: K.Keyword, insertText: k, range, sortText: `5${k}` });
    if (atLineStart) {
      for (const [label, what, body] of SNIPPETS) out.push({ label: { label, description: "snippet" }, kind: K.Snippet, detail: what, documentation: { value: "```pawn\n" + body.replace(/\$\{\d+:([^}]*)\}/g, "$1").replace(/\$\d/g, "") + "\n```" }, insertText: body, insertTextRules: snippet, range, sortText: `1${label}` });
    }
  }
  return { suggestions: out };
}

/** The "/" menu: snippets, whole templates (in an empty file), a new plugin, the docs. */
function palette(model: monaco.editor.ITextModel, range: monaco.Range, slash: boolean): monaco.languages.CompletionItem[] {
  const snippet = monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet;
  const pre = slash ? "/" : "";
  const out: monaco.languages.CompletionItem[] = SNIPPETS.map(([label, what, body], i) => ({
    label: { label: `${pre}${label}`, description: what },
    kind: K.Snippet,
    detail: what,
    documentation: { value: "```pawn\n" + body.replace(/\$\{\d+:([^}]*)\}/g, "$1").replace(/\$\d/g, "") + "\n```" },
    insertText: body,
    insertTextRules: snippet,
    filterText: `${pre}${label} ${what}`,
    range,
    sortText: `1${String(i).padStart(3, "0")}`,
  }));
  const empty = model.getValue().trim().length < 3;
  for (const t of TEMPLATES) {
    if (!empty) break;
    const name = model.uri.path.split("/").pop()?.replace(/\.[^.]+$/, "") || "my_plugin";
    out.push({ label: { label: `${pre}template ${t.label}`, description: "whole plugin" }, kind: K.File, detail: t.blurb, insertText: t.text(name), filterText: `${pre}template ${t.label}`, range, sortText: `0${t.id}` });
  }
  if (cmdIds.newPlugin) out.push({ label: { label: `${pre}new plugin`, description: "from a template" }, kind: K.File, detail: "Make a new plugin in My plugins from a template", insertText: "", filterText: `${pre}new plugin template`, range, sortText: "2new", command: { id: cmdIds.newPlugin, title: "New plugin" } });
  if (cmdIds.docs) out.push({ label: { label: `${pre}docs`, description: "search every function" }, kind: K.Reference, detail: "Search the docs: every native, forward, stock and constant, and the guides", insertText: "", filterText: `${pre}docs help search`, range, sortText: "2docs", command: { id: cmdIds.docs, title: "Docs" } });
  return out;
}

/** Opens the "/" menu at the cursor (the Insert button). */
export function openPalette(ed: monaco.editor.IStandaloneCodeEditor) {
  paletteNext = true;
  ed.focus();
  ed.trigger("palette", "editor.action.triggerSuggest", {});
}

// ---- go to definition ----------------------------------------------------------------

/** An include's model (read only), so peek and go to definition can show it. */
async function includeModel(file: string): Promise<monaco.Uri | null> {
  const uri = monaco.Uri.parse(`inmemory://plugins/include/${file.split("/").map(encodeURIComponent).join("/")}`);
  if (monaco.editor.getModel(uri)) return uri;
  const text = await hooks?.readInclude(file);
  if (text == null) return null;
  if (!monaco.editor.getModel(uri)) models.set(`include/${file}`, monaco.editor.createModel(text, LANG, uri));
  return uri;
}

async function definition(model: monaco.editor.ITextModel, pos: monaco.Position): Promise<monaco.languages.Definition | null> {
  const w = model.getWordAtPosition(pos);
  if (!w) return null;
  const l = locals(model).find((x) => x.name === w.word);
  if (l) return { uri: model.uri, range: new monaco.Range(l.line, 1, l.line, 1) };
  const s = byName.get(w.word);
  if (!s) return null;
  const uri = await includeModel(s.file);
  return uri ? { uri, range: new monaco.Range(s.line, 1, s.line, 1) } : null;
}

// ---- hover ---------------------------------------------------------------------------

function hover(model: monaco.editor.ITextModel, pos: monaco.Position): monaco.languages.Hover | null {
  const w = model.getWordAtPosition(pos);
  if (!w) return null;
  const range = new monaco.Range(pos.lineNumber, w.startColumn, pos.lineNumber, w.endColumn);
  const s = byName.get(w.word);
  if (s) return { range, contents: [docOf(s)] };
  const l = locals(model).find((x) => x.name === w.word);
  if (l && l.line !== pos.lineNumber) return { range, contents: [{ value: "```pawn\n" + l.text + "\n```" }, { value: `Defined on line ${l.line} of this file.` }] };
  return null;
}

// ---- signature help ------------------------------------------------------------------

/** The function call the cursor is in (its name) and which argument it's on. */
export function callAt(text: string): { name: string; arg: number } | null {
  let depth = 0;
  let arg = 0;
  let quote = false;
  for (let i = text.length - 1; i >= 0 && i > text.length - 4000; i--) {
    const c = text[i];
    if (c === '"' && text[i - 1] !== "^") {
      quote = !quote;
      continue;
    }
    if (quote) continue;
    if (c === ")" || c === "]" || c === "}") depth++;
    else if (c === "[" || c === "{") {
      if (depth === 0) return null;
      depth--;
    } else if (c === "(") {
      if (depth === 0) {
        const m = text.slice(0, i).match(/([A-Za-z_@][\w@]*)\s*$/);
        return m ? { name: m[1], arg } : null;
      }
      depth--;
    } else if (c === "," && depth === 0) arg++;
    else if (c === ";" && depth === 0) return null;
  }
  return null;
}

function signature(model: monaco.editor.ITextModel, pos: monaco.Position): monaco.languages.SignatureHelpResult | null {
  const text = model.getValueInRange(new monaco.Range(Math.max(1, pos.lineNumber - 30), 1, pos.lineNumber, pos.column));
  const call = callAt(text);
  if (!call) return null;
  const s = byName.get(call.name);
  if (!s || !FUNCS.has(s.kind)) return null;
  const variadic = s.params.length > 0 && s.params[s.params.length - 1].name === "...";
  const active = variadic ? Math.min(call.arg, s.params.length - 1) : call.arg;
  const label = `${s.name}(${s.params.map((p) => p.label).join(", ")})`;
  return {
    value: {
      signatures: [{ label, documentation: s.doc ? { value: s.doc } : undefined, parameters: s.params.map((p) => ({ label: p.label, documentation: p.doc ? { value: p.doc } : undefined })) }],
      activeSignature: 0,
      activeParameter: active,
    },
    dispose() {},
  };
}

// ---- the editor ----------------------------------------------------------------------

const models = new Map<string, monaco.editor.ITextModel>();

/** The file's model (kept, with its undo history, until the page is reloaded). */
export function modelFor(key: string, text: string | null): monaco.editor.ITextModel {
  setup();
  let m = models.get(key);
  if (!m) {
    m = monaco.editor.createModel(text ?? "", LANG, monaco.Uri.parse(`inmemory://plugins/${key}`));
    models.set(key, m);
  } else if (text !== null && m.getValue() !== text) {
    m.pushEditOperations([], [{ range: m.getFullModelRange(), text }], () => null);
  }
  return m;
}

export function dropModel(key: string) {
  models.get(key)?.dispose();
  models.delete(key);
}

export type EditorHandle = {
  show: (key: string, text: string | null, readOnly: boolean) => void;
  value: () => string;
  markers: (diags: Diag[]) => void;
  reveal: (line: number) => void;
  layout: () => void;
  palette: () => void;
  /** puts a word from the include files at the cursor (a call with its parameters to fill in) */
  insertWord: (w: Word) => void;
  focus: () => void;
  dispose: () => void;
};

export function createEditor(el: HTMLElement, on: { change: () => void; save: () => void; compile: () => void; hooks: PageHooks }): EditorHandle {
  setup();
  hooks = on.hooks;
  const ed = monaco.editor.create(el, {
    theme: "ls-pawn",
    automaticLayout: true,
    fontSize: 14,
    fontFamily: "Menlo, Consolas, 'DejaVu Sans Mono', monospace",
    minimap: { enabled: false },
    // (its header kept the last file's line when switching files)
    stickyScroll: { enabled: false },
    tabSize: 4,
    insertSpaces: false,
    scrollBeyondLastLine: false,
    renderWhitespace: "selection",
    fixedOverflowWidgets: true,
    suggest: { showStatusBar: true, preview: true },
    quickSuggestionsDelay: 80,
    wordBasedSuggestions: "off",
    model: null,
  });
  const sub = ed.onDidChangeModelContent(() => on.change());
  cmdIds = { newPlugin: ed.addCommand(0, () => hooks?.newPlugin()) ?? undefined, docs: ed.addCommand(0, () => hooks?.docs("")) ?? undefined };
  ed.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => on.save());
  ed.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => on.compile());
  return {
    show(key, text, readOnly) {
      ed.setModel(modelFor(key, text));
      ed.updateOptions({ readOnly });
    },
    value: () => ed.getValue(),
    markers(diags) {
      const m = ed.getModel();
      if (!m) return;
      monaco.editor.setModelMarkers(
        m,
        "amxxpc",
        diags.map((d) => {
          const line = Math.min(Math.max(1, d.line), m.getLineCount());
          return {
            severity: d.severity === "warning" ? monaco.MarkerSeverity.Warning : monaco.MarkerSeverity.Error,
            message: d.plain ? `${d.plain}\n(compiler: ${d.severity} ${d.code}: ${d.message})` : d.message,
            startLineNumber: line,
            endLineNumber: line,
            startColumn: m.getLineFirstNonWhitespaceColumn(line) || 1,
            endColumn: m.getLineMaxColumn(line),
          };
        }),
      );
    },
    reveal(line) {
      ed.revealLineInCenter(line);
      ed.setPosition({ lineNumber: line, column: 1 });
      ed.focus();
    },
    layout: () => ed.layout(),
    palette: () => openPalette(ed),
    insertWord(w) {
      if (ed.getOption(monaco.editor.EditorOption.readOnly)) return;
      const text = FUNCS.has(w.kind) && w.kind !== "forward" ? (w.kind === "macro" ? `${w.name}(\${1})` : callSnippet(w)) : w.kind === "forward" ? `public ${w.name}(${w.params.map((p) => p.label).join(", ")})\n{\n\t$0\n}` : w.name;
      ed.focus();
      (ed.getContribution("snippetController2") as unknown as { insert: (t: string) => void } | null)?.insert(text);
    },
    focus: () => ed.focus(),
    dispose() {
      sub.dispose();
      ed.dispose();
      hooks = null;
      cmdIds = {};
    },
  };
}
