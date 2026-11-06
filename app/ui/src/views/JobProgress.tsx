import { useEffect, useRef, useState } from "react";
import { clock, usePoll, useStable } from "../api";
import { useSetupJob } from "../queries";
import { SteamProgress } from "../ui";

/** The setup job's progress. A failed job stays on screen with why it stopped until
 *  Close (and Try again, when the caller can start it again). */
export function JobProgress({ onEnd: onEndProp, onClose, onRetry, title, remote }: {
  onEnd: (ok: boolean) => void;
  onClose?: () => void;
  onRetry?: () => void;
  /** what it's doing, e.g. "Updating Valve's server files…" */
  title?: string;
  /** on another machine: setup there takes 5 to 15 minutes */
  remote?: boolean;
}) {
  const onEnd = useStable(onEndProp);
  const q = useSetupJob();
  // a job that had already ended before this bar opened isn't this one
  const [startId] = useState(() => q.data?.id);
  const seen = useRef(false);
  const ended = useRef(false);
  const [lost, setLost] = useState(false);
  const [opened] = useState(() => Date.now());
  const [, tick] = useState(0);
  const data = q.isFetchedAfterMount ? q.data : undefined;
  const job = data && (data.state === "running" || seen.current || (data.state !== "idle" && data.id !== startId)) ? data : null;
  if (job) seen.current = true;
  const over = !!job && (job.state === "done" || job.state === "error");

  useEffect(() => {
    if (!over || ended.current) return;
    ended.current = true;
    onEnd(job!.state === "done");
  }, [over, job, onEnd]);
  // the shared query only polls a running job: ask until this one shows up, and give up
  // (saying so) if it never does or the app stops answering
  const latest = useRef(q);
  latest.current = q;
  const misses = useRef(0);
  usePoll(async () => {
    const cur = latest.current;
    const d = cur.isFetchedAfterMount ? cur.data : undefined;
    if (cur.isError || !seen.current || d?.state !== "running") {
      if (++misses.current >= 15 && !ended.current) {
        ended.current = true;
        setLost(true);
        onEnd(false);
      }
      if (d?.state !== "running") await cur.refetch();
    } else misses.current = 0;
    // the elapsed time moves on
    tick((n) => n + 1);
  }, 1000, [], !over && !lost);

  // the job's id ends in when it started (ms)
  const began = Number(/-(\d{12,})$/.exec(job?.id ?? "")?.[1]) || opened;
  const elapsed = Math.max(0, Math.round((Date.now() - began) / 1000));
  const failed = lost || job?.state === "error";
  const steps = job?.steps ?? [];
  const now = steps.find((s) => s.state === "running");
  const n = now ? steps.indexOf(now) + 1 : steps.filter((s) => s.state === "done").length;
  const foot = failed
    ? ""
    : [steps.length > 1 && n > 0 ? `Step ${n} of ${steps.length}` : "", `${clock(elapsed)} so far`, remote ? "usually 5 to 15 minutes" : ""].filter(Boolean).join(" · ");
  const name = title ?? (job?.kind === "server" ? "Setting up the server…" : "Installing…");
  return (
    <div className="job-progress">
      <SteamProgress
        value={job?.progress ?? 0}
        title={failed ? `Stopped: ${name.charAt(0).toLowerCase()}${name.slice(1).replace(/…$/, "")}` : name}
        text={lost ? "Lost track of it. The app stopped answering; check the server's page to see if it finished." : job?.state === "error" ? job.error : now ? `${now.label}${now.detail ? ` · ${now.detail}` : ""}` : "Getting ready…"}
        error={failed}
        foot={foot}
      />
      {failed && (onRetry || onClose) && (
        <div className="row">
          {onRetry && <button className="btn primary" onClick={onRetry}>Try again</button>}
          {onClose && <button className="btn" onClick={onClose}>Close</button>}
        </div>
      )}
    </div>
  );
}
