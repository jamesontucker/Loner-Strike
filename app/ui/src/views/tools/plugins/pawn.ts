// Pawn (AMX Mod X's language) for Monaco: a Monarch grammar, brackets and comments, and
// the editor colours in the app's palette. Types only from monaco here: this file is
// loaded with the editor's own chunk.

import type { editor, languages } from "monaco-editor/esm/vs/editor/editor.api";

export const KEYWORDS = [
  "new", "public", "stock", "static", "const", "native", "forward", "enum", "if", "else", "for", "while", "do", "switch", "case",
  "default", "return", "break", "continue", "goto", "sizeof", "tagof", "charsmax", "defined", "operator", "state", "sleep", "assert",
  "exit", "char", "true", "false",
];

/** The grammar. `functions` and `constants` (from the include files) get their own colours. */
export function pawnGrammar(functions: string[], constants: string[]): languages.IMonarchLanguage {
  return {
    defaultToken: "",
    tokenPostfix: ".pawn",
    keywords: KEYWORDS,
    functions,
    constants,
    operators: ["=", ">", "<", "!", "~", "?", ":", "==", "<=", ">=", "!=", "&&", "||", "++", "--", "+", "-", "*", "/", "&", "|", "^", "%", "<<", ">>", ">>>", "+=", "-=", "*=", "/=", "&=", "|=", "^=", "%=", "<<=", ">>=", "..."],
    symbols: /[=><!~?:&|+\-*/^%.]+/,
    // AMX Mod X escapes with ^ (^n, ^t, ^", ^^, ^x01)
    escapes: /\^(?:[abefnrtv"'^]|x[0-9A-Fa-f]{1,4};?|\d{1,3};?)/,
    tokenizer: {
      root: [
        [/^(\s*#\s*include|\s*#\s*tryinclude)(\s*)([<"][^>"]*[>"]?)/, ["keyword.directive", "", "string.include"]],
        [/^\s*#\s*[a-zA-Z_]+/, "keyword.directive"],
        // a tag: Float:x, bool:done, CsTeams:team (not ::)
        [/[A-Za-z_@][\w@]*(?=:(?!:))/, "type"],
        [/[A-Za-z_@][\w@]*(?=\s*\()/, { cases: { "@keywords": "keyword", "@functions": "function.known", "@default": "function" } }],
        [/[A-Za-z_@][\w@]*/, { cases: { "@keywords": "keyword", "@constants": "constant", "@default": "identifier" } }],
        { include: "@whitespace" },
        [/[{}()[\]]/, "@brackets"],
        [/\d*\.\d+([eE][-+]?\d+)?/, "number.float"],
        [/\d+\.\d*([eE][-+]?\d+)?/, "number.float"],
        [/0[xX][0-9a-fA-F_]+/, "number.hex"],
        [/0[bB][01_]+/, "number"],
        [/\d[\d_]*/, "number"],
        [/@symbols/, { cases: { "@operators": "operator", "@default": "" } }],
        [/[;,]/, "delimiter"],
        [/"/, { token: "string.quote", next: "@string" }],
        [/'(?:[^'^\\]|\^.|\\.)'/, "string"],
      ],
      string: [
        [/[^"^]+/, "string"],
        [/@escapes/, "string.escape"],
        [/\^./, "string.escape"],
        [/"/, { token: "string.quote", next: "@pop" }],
      ],
      whitespace: [
        [/[ \t\r\n]+/, ""],
        [/\/\*\*(?!\/)/, "comment.doc", "@doc"],
        [/\/\*/, "comment", "@comment"],
        [/\/\/.*$/, "comment"],
      ],
      comment: [
        [/[^/*]+/, "comment"],
        [/\*\//, "comment", "@pop"],
        [/[/*]/, "comment"],
      ],
      doc: [
        [/@\w+/, "comment.doc.tag"],
        [/[^/*@]+/, "comment.doc"],
        [/\*\//, "comment.doc", "@pop"],
        [/[/*@]/, "comment.doc"],
      ],
    },
  } as languages.IMonarchLanguage;
}

export const pawnConfig: languages.LanguageConfiguration = {
  comments: { lineComment: "//", blockComment: ["/*", "*/"] },
  brackets: [["{", "}"], ["[", "]"], ["(", ")"]],
  autoClosingPairs: [
    { open: "{", close: "}" },
    { open: "[", close: "]" },
    { open: "(", close: ")" },
    { open: '"', close: '"', notIn: ["string", "comment"] },
    { open: "'", close: "'", notIn: ["string", "comment"] },
    { open: "/**", close: " */", notIn: ["string"] },
  ],
  surroundingPairs: [
    { open: "{", close: "}" },
    { open: "[", close: "]" },
    { open: "(", close: ")" },
    { open: '"', close: '"' },
  ],
  wordPattern: /(-?\d*\.\d\w*)|([^`~!#%^&*()\-=+[{\]}\\|;:'",.<>/?\s]+)/g,
  indentationRules: {
    increaseIndentPattern: /^((?!\/\/).)*(\{[^}"'`]*|\([^)"'`]*|\[[^\]"'`]*)$/,
    decreaseIndentPattern: /^((?!.*?\/\*).*\*\/)?\s*[}\])].*$/,
  },
  folding: { markers: { start: /^\s*#if\b|^\s*\/\/\s*#?region\b/, end: /^\s*#endif\b|^\s*\/\/\s*#?endregion\b/ } },
};

/** The app's olive Steam colours, for the editor. */
export const pawnTheme: editor.IStandaloneThemeData = {
  base: "vs-dark",
  inherit: true,
  rules: [
    { token: "", foreground: "d8ded3" },
    { token: "keyword", foreground: "c4b550", fontStyle: "bold" },
    { token: "keyword.directive", foreground: "d9b25a" },
    { token: "type", foreground: "8fb4d6" },
    { token: "function", foreground: "e8e0b8" },
    { token: "function.known", foreground: "f0d98a" },
    { token: "constant", foreground: "d99a7a" },
    { token: "identifier", foreground: "d8ded3" },
    { token: "number", foreground: "d9a441" },
    { token: "string", foreground: "b3c77f" },
    { token: "string.include", foreground: "b3c77f" },
    { token: "string.escape", foreground: "e6c97a" },
    { token: "comment", foreground: "8f9a80", fontStyle: "italic" },
    { token: "comment.doc", foreground: "9aa88a", fontStyle: "italic" },
    { token: "comment.doc.tag", foreground: "b9c49a" },
    { token: "operator", foreground: "c2c9b7" },
    { token: "delimiter", foreground: "aeb7a2" },
  ],
  colors: {
    "editor.background": "#2b3126",
    "editor.foreground": "#d8ded3",
    "editorLineNumber.foreground": "#6f7a62",
    "editorLineNumber.activeForeground": "#c4b550",
    "editor.lineHighlightBackground": "#343b2e",
    "editor.selectionBackground": "#5a6a5088",
    "editorCursor.foreground": "#c4b550",
    "editorIndentGuide.background1": "#3b4334",
    "editorWidget.background": "#353c2f",
    "editorWidget.border": "#292c21",
    "editorSuggestWidget.background": "#353c2f",
    "editorSuggestWidget.selectedBackground": "#4c5844",
    "editorHoverWidget.background": "#353c2f",
    "editorError.foreground": "#e88a6a",
    "editorWarning.foreground": "#d9b25a",
  },
};
