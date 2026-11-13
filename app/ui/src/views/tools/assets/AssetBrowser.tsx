// Textures & models (Tools, Beta): every texture in the game's WADs and inside its maps,
// every model in 3D, and adding your own or getting more from GameBanana.

import { useState } from "react";
import { Segmented } from "../../../ui";
import type { ToolProps } from "../registry";
import { GetAssets } from "./GetAssets";
import { Models } from "./Models";
import { Textures } from "./Textures";
import "./assets.css";

type Tab = "textures" | "models" | "get";

export default function AssetBrowser(_: ToolProps) {
  const [tab, setTab] = useState<Tab>("textures");
  return (
    <div className="assets">
      <div className="row between assets-head">
        <Segmented values="textures models get" labels="Textures|Models|Get more" current={tab} onPick={(v) => setTab(v as Tab)} label="Show" />
        <span className="muted assets-note">Browsing changes nothing in your game.</span>
      </div>
      {tab === "textures" && <Textures />}
      {tab === "models" && <Models />}
      {tab === "get" && <GetAssets />}
    </div>
  );
}
