// P5 (QA4-A): the map name check the Compile panel says before a compile (the app checks the same:
// compile.rs map_name_problem). Tested by scripts/mapname.test.ts.
/** What's wrong with a map name ("" = nothing; empty = the suggested one): the app's own check
 *  (compile.rs map_name_problem), said before a compile starts. P5 (QA4-A). */
export function mapNameProblem(n: string): string {
  if (!n) return "";
  if (!/^[A-Za-z0-9_-]{1,31}$/.test(n)) return "Map names can only use letters, numbers, _ and - (up to 31).";
  if (!/[A-Za-z0-9]/.test(n)) return "A map name needs at least one letter or number.";
  if (/^(con|prn|aux|nul|com[0-9]|lpt[0-9])$/i.test(n)) return "Windows keeps that name for a device (like CON, NUL, AUX or COM1), so the map couldn't be saved. Pick another name.";
  return "";
}
