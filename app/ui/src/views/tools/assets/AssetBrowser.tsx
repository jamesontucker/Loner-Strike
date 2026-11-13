// Textures & models (Tools, Beta): every texture in the game's WADs and inside its maps,
// every model in 3D, and adding your own or getting more from GameBanana.

import { useState } from "react";
import { useToolArg } from "../../../stores/tools";
import { Segmented } from "../../../ui";
import { useReportAddress } from "../common/address";
import { useToolCrumbs } from "../common/shell";
import type { ToolProps } from "../registry";
import { GetAssets } from "./GetAssets";
import { Models } from "./Models";
import { Textures } from "./Textures";
import "./assets.css";

type Tab = "textures" | "models" | "get";

export default function AssetBrowser(_: ToolProps) {
  // "assets:models", "assets:get": the tab is part of the address (Back steps between tabs)
  const arg = useToolArg();
  const [tab, setTab] = useState<Tab>(arg === "models" || arg === "get" ? arg : "textures");
  useReportAddress("assets", tab === "textures" ? null : `assets:${tab}`);
  useToolCrumbs("assets", [{ label: tab === "textures" ? "Textures" : tab === "models" ? "Models" : "Get more" }]);
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
