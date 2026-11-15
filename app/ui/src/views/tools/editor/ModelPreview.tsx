// A model an entity names, in the asset browser's 3D viewer (its own chunk): looked for in
// the game's folders in the order the game reads them.

import { useEffect, useState } from "react";
import { getModel, type ModelBundle } from "../assets/assetsApi";
import { ModelView } from "../assets/ModelView";

const FOLDERS = ["cstrike", "cstrike_downloads", "czero", "valve"];

export default function ModelPreview({ path }: { path: string }) {
  const [got, setGot] = useState<{ item: { folder: string; path: string }; bundle: ModelBundle } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lost, setLost] = useState(0);
  useEffect(() => {
    const ac = new AbortController();
    setGot(null);
    setError(null);
    const rel = path.replace(/\\/g, "/").replace(/^\/+/, "");
    void (async () => {
      for (const folder of FOLDERS) {
        try {
          const item = { folder, path: rel };
          const bundle = await getModel(item, ac.signal);
          if (!ac.signal.aborted) setGot({ item, bundle });
          return;
        } catch {
          if (ac.signal.aborted) return;
        }
      }
      setError(`${rel} isn't in the game's folders, or it couldn't be read.`);
    })();
    return () => ac.abort();
  }, [path]);
  if (error) return <span className="muted">{error}</span>;
  if (!got) return <span className="muted"><span className="spinner" aria-hidden="true" /> Opening the model…</span>;
  return <ModelView key={lost} item={got.item} bundle={got.bundle} skin={0} body={[]} anim={null} playing={false} wire={false} onLost={() => setTimeout(() => setLost((n) => n + 1), 500)} />;
}
