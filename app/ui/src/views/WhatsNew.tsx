import { useEffect, useState } from "react";
// the changelog, bundled with the app: what's new in this version, without asking the internet
import changelog from "../../../../CHANGELOG.md?raw";

/** A version's section of CHANGELOG.md ("## 0.1.3" up to the next "## "), or "". */
export function changelogFor(version: string): string {
  const lines = changelog.split(/\r?\n/);
  const at = lines.findIndex((l) => l.trim() === `## ${version}`);
  if (at < 0) return "";
  const out: string[] = [];
  for (const l of lines.slice(at + 1)) {
    if (l.startsWith("## ")) break;
    out.push(l);
  }
  return out.join("\n").trim();
}

/** Release notes as the changelog writes them: "### " headings, "- " points, plain lines.
 * (A release's notes end with download help after a "---" line, which isn't shown here.) */
export function ReleaseNotes({ text }: { text: string }) {
  const body = text.split(/\n---\s*\n/)[0].trim();
  const blocks: JSX.Element[] = [];
  let points: string[] = [];
  const flush = () => {
    if (points.length) blocks.push(<ul key={`u${blocks.length}`} className="plain-list notes-list">{points.map((p, i) => <li key={i}>{p}</li>)}</ul>);
    points = [];
  };
  for (const raw of body.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) {
      flush();
      continue;
    }
    if (line.startsWith("- ")) points.push(line.slice(2));
    else if (line.startsWith("### ")) {
      flush();
      blocks.push(<h4 key={`h${blocks.length}`} className="notes-head">{line.slice(4)}</h4>);
    } else {
      flush();
      blocks.push(<p key={`p${blocks.length}`}>{line}</p>);
    }
  }
  flush();
  return <div className="release-notes">{blocks}</div>;
}

const SEEN = "ls-seen-version";

/** The first time a new version opens: what changed, once, until dismissed. Not on a first
 * install (there's nothing it was new to). */
export function WhatsNew({ version }: { version?: string }) {
  const [show, setShow] = useState(false);
  useEffect(() => {
    if (!version) return;
    let seen: string | null = null;
    try {
      seen = localStorage.getItem(SEEN);
    } catch {
      return;
    }
    if (seen === null) {
      try {
        localStorage.setItem(SEEN, version);
      } catch {
        /* storage off: nothing to remember it in */
      }
      return;
    }
    if (seen !== version && changelogFor(version)) setShow(true);
  }, [version]);
  if (!show || !version) return null;
  const dismiss = () => {
    try {
      localStorage.setItem(SEEN, version);
    } catch {
      /* storage off */
    }
    setShow(false);
  };
  return (
    <section className="panel whats-new" aria-label={`What's new in ${version}`}>
      <div className="row between">
        <h3>What's new in {version}</h3>
        <button className="btn" onClick={dismiss}>Got it</button>
      </div>
      <ReleaseNotes text={changelogFor(version)} />
    </section>
  );
}
