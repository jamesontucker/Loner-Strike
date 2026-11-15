# Drawing maps like the game (map viewer, map editor)

How Counter-Strike 1.6 / Condition Zero (GoldSrc, OpenGL renderer) draws a map, what the Tools
viewer does to match it, how to check it against the game, and what's still different.
Agent K, 2026-11-12.

## Sources

All code here is our own; these were read for the behaviour and the maths.

- **Xash3D FWGS** (GPL-3, https://github.com/FWGS/xash3d-fwgs, read at the `master` of
  2026-11-12), a GoldSrc-compatible engine whose gamma tables are unit-tested against tables
  taken from GoldSrc itself (`engine/client/gamma.c`, `precomputed_gamma_tables_s`):
  - `engine/client/gamma.c` `BuildGammaTable`, `TextureToGamma`, `LightToTexGamma`, cvar defaults
  - `ref/gl/gl_rsurf.c` `R_BuildLightMap` (style sum, light scale, cap, table), `R_SetRenderMode`,
    `R_HasLightmap`, `R_DrawAlphaTextureChains`, `EmitWaterPolys`, `R_RenderBrushPoly`
  - `ref/gl/gl_backend.c` `GL_SetRenderMode`; `ref/gl/gl_rmain.c` `CL_FxBlend` (renderfx)
  - `ref/common/ref_light.c` `CL_RunLightStyles` (10 Hz, `* 22`), `R_RecursiveLightPoint`,
    `R_LightVec`, `R_EntityDynamicLight` (model light: sky, floor, gradient, ambient/shade split)
  - `ref/gl/gl_studio.c` `R_StudioLighting` (`SHADE_LAMBERT` 1.4953241)
  - `ref/gl/gl_warp.c` `st_to_vec`, `MakeSkyVec`; `ref/common/ref_skybox.c` `r_skyTexOrder`;
    `engine/client/dll_int/ref_common.c` sky suffixes
  - `engine/common/imagelib/img_wad.c` (texgamma on WAD textures, `{` masked palette)
- **ReGameDLL_CS** (the user's local checkout): `dlls/world.cpp` (the 13 built-in light style
  strings), `dlls/lights.cpp` (switchable lights: style ≥ 32, "start off" flag, `pattern`;
  `light_environment` → `sv_skycolor` `pow(c/114, 0.6) * 264`, `sv_skyvec`). Only the facts
  (strings, formula, key names) are used.
- The player's own `cstrike/config.cfg` (and `czero/`): `gamma "2.5"`, `brightness "1"`,
  `gl_overbright "0"`, `gl_texturemode "GL_LINEAR_MIPMAP_LINEAR"`, `gl_ansio "16"`.
- TWHL / Valve Developer wiki knowledge for mapper-facing rules (render modes, `{` textures
  needing a Solid-mode entity), checked against the code above.

## How the game draws it

**Textures.** WAD/embedded miptex palettes go through the texgamma table on upload:
`c' = floor(255 * (c/255)^(texgamma/gamma))` (defaults 2.0 / 2.5 → exponent 0.8, a lift).
`{` textures: palette index 255 becomes (0,0,0,0); the GL mipmaps are made from that, so
fence edges darken. Filtering is `gl_texturemode` (default trilinear) with `gl_ansio`.

**Lightmaps.** One RGB luxel per 16 texture units (extents as agent A wrote them). A face has
up to 4 pictures, one per style in `styles[0..3]` (255 = none), back to back in the lump.
Per luxel, every frame:

    acc   = Σ sample_k * style_value(styles[k])          (style 'a'=0 … 'm'=264 … 'z'=550)
    scale = gl_overbright ? 256 : round(2^(1/lightgamma) * 256)    (= 338 at lightgamma 2.5)
    t     = min(1023, (acc * scale) >> 14)
    out   = lightgamma_table[t] >> 2                       (0..255, the GL lightmap texel)

    lightgamma_table[i]: f = (i/1023)^lightgamma; if brightness > 1: f *= brightness;
        g3 = brightness <= 0 ? 0.125 : brightness <= 1 ? 0.125 - 0.075*brightness² : 0.05
        f = f <= g3 ? f/g3 * 0.125 : (f-g3)/(1-g3) * 0.875 + 0.125
        → floor(1023 * f^(1/gamma))

The surface is `texture * out` (GL_MODULATE), and with `gl_overbright 1` once more ×2 (blend
`DST_COLOR, SRC_COLOR`). So without overbright a luxel of ~128 already gives the full texture
and brighter light clamps; the old viewer's "×2 overbright" was too bright and too flat.

**Light styles.** `value = (letter - 'a') * 22`, stepping at 10 letters a second, no blending.
Styles 0-12 and 63 come from the game DLL (`m`, flicker `mmnmmommommnonmmonqnmmo`, …; CS's
style 7 differs from Quake's). Lights with `style` ≥ 32 are switchable: `a` when spawnflag 1
("initially dark"), else their `pattern` or `m`. A style nobody set is 256.

**Fullbright cases.** Faces with no lightmap (sky, water, `TEX_SPECIAL`); `r_fullbright 1`; a
map with no lighting lump; brush entities in render modes 1, 2, 3, 5 (no lightmap pass).

**Water** (`!`, `*`): no lightmap, texture coordinates
`s = (os + 8 sin(ot/8 + t)) / 64`, `t = (ot + 8 sin(os/8 + t)) / 64` (os, ot in texture units),
i.e. one repeat per 64 units whatever the texture size. Opaque on the world; a `func_water`
uses its render mode (CS maps: mode 2 with renderamt 50-100). (Vertex waves come from the
entity's `scale`/WaveHeight; the stock maps have none.)

**Sky.** Sky faces aren't drawn; the box `gfx/env/<skyname>{rt,bk,lf,ft,up,dn}.tga` is (TGA, no
gamma). Side layout (`st_to_vec`): +x shows rt (+y on its left), −x lf, +y bk, −y ft, up has −x
at its top and +y on its left, dn +x at its top. Note the swap: the game binds picture
`r_skyTexOrder = {0,2,1,3,4,5}` to box side i.

**Render modes** (brush entities; `renderamt` 0..255):
0 normal; 1 colour: `rendercolor` with alpha amt, no texture, no lightmap; 2 texture and
3 glow: texture with alpha `amt`, no lightmap, no depth write; 4 solid: alpha test
`> 0.25`, lit, opaque; 5 additive: `texture * amt` added (`ONE, ONE`), no lightmap. A mode
other than 0 with amt 0 isn't drawn. `{` textures on the world or a mode-0 entity aren't
see-through: their clear pixels are black. `renderfx` pulses/strobes modulate amt (not used
by the stock maps' brushes; not done).

**Models.** Light at the model: from 8 units above its origin, if `light_environment` set
`sv_skycolor` and a line towards the sun meets sky first, the sun's colour and direction;
else the nearest luxel under it (2048 units down, styles summed, `>> 8`, cap 255) with a
direction from four samples 16 units around. `total = max(r,g,b)`, `shade = 0.9 total`,
`ambient = total - shade` (≤ 128, `ambient + shade ≤ 255`), colour = light / total. Per vertex:
`cos = (dot(n, dir) + 0.4953) / 1.4953`; `illum = ambient + shade - (cos > 0 ? shade*cos : 0)`;
`light = lightgamma_table[4 * illum] / 1023`; pixel = texgamma(texture) * light * colour.
Full-bright textures skip it; masked ones alpha-test, additive ones add.

## What the viewer does (files)

- `app/src-tauri/src/goldsrc/lightmap.rs`: the atlas keeps **every style**: planes stacked
  top to bottom (`maps` picture planes, then a plane of the face's style numbers R,G,B = styles
  0,1,2, and a second (R = style 3) only when some face has four). `uv2` unchanged (relative to
  one plane). `light_point` / `first_hit`: the game's light sample under a point. Header
  `lightmap: {width, height, maps, planes}`; `PARSER_VERSION` 2 rebuilds old caches.
- `app/src-tauri/src/tools/render.rs`: `GET /api/tools/map/:name/look` (settings from the
  map's game folder `config.cfg`/`userconfig.cfg`/`autoexec.cfg`, clamped like the engine; sky
  name and which sides exist; the sun; per model entity: model, folder, light, direction,
  sky) and `GET /api/tools/map/:name/sky/:side` (TGA/BMP → PNG, size-capped; the name comes
  from the map, never the page).
- `app/ui/src/views/tools/viewer/gameLook.ts`: the tables in JS, style strings and values,
  model light, the surface shader (texgamma, style sum through the table, water warp, render
  modes, alpha test) and the sky shader (direction → side and spot, one 6-slot texture).
- `viewer/materials.ts`: `MapMaterials` (same constructor and `get(d)` as before; new optional
  6th argument `lightmapInfo`, `get(d, render)`, `setSettings`, `setSky`, `uniforms`). With
  Lighting on: the game's look; off: the old sun shading (the editor uses this, unchanged).
- `viewer/World.tsx`: fetches `/look`, loads the sky, passes each brush entity's render mode,
  runs `GameClock` (rAF only while a style animates or water is in the map, page visible,
  Lighting on). `viewer/Models.tsx`: studio models placed in the map, lit as above.
- `viewer/LookChoice.tsx` + `look.ts`: Layers ▸ under Lighting, **Look: Like the game | Bright**
  (Bright = brightness 3 + overbright; remembered per device).
- Dev only: `viewer/DevCamera.tsx` (`window.__lsViewer.camera(x,y,z,pitch,yaw,fov)` and
  `.shot()`), `app/ui/render-dev.html` + `viewer/renderDev.tsx` (the viewer alone, full window,
  `?map=&cam=x,y,z,pitch,yaw&look=`), `render.rs` test `serve_tools_for_ui` (all Tools routes on
  :8027 without the app).

## Checking against the game

Viewer side (no app needed):

    cd app/src-tauri && LS_TEST_GAME="<Half-Life folder>" cargo test --release serve_tools_for_ui -- --ignored --nocapture
    cd app/ui && npx vite      # then, at 1024x768:
    http://localhost:5173/render-dev.html?map=de_dust2&cam=-832,-800,176,0,10

`cam` is the player's origin (as `setpos` takes it); the eye is 17 units above; fov 90 is the
game's (horizontal at 4:3, the 4:3 height kept on wider screens, as `gl_widescreen_yfov 1`).

Game side (the user's CS 1.6 in CrossOver):
1. Compile `docs/render-compare/ls_camera.sma` with the game's AMXX compiler and add
   `ls_camera.amxx` to `plugins.ini` (GoldSrc has no `setpos`/`setang`; this does the same
   with noclip, cheats only).
2. Copy `docs/render-compare/lscompare.cfg` into `cstrike/`.
3. `map de_dust2`, join any team, `exec lscompare.cfg`, then `ls_d2_spawn`, `ls_d2_sky`,
   `ls_d2_tunnel`; same for cs_office, cs_havana, cs_747, de_aztec (aliases in the cfg).
   Windowed 1024x768 makes the pictures line up with the viewer's.
4. Screenshots are `cstrike/<map>NNNN.bmp`; open the matching render-dev URL (same numbers)
   and compare side by side.

## Results (2026-11-12)

Viewer pictures taken at the cfg's spots (render-dev, 1024x768, the user's settings: gamma
2.5, brightness 1, texgamma 2, lightgamma 2.5, no overbright, trilinear, aniso 16):

| Spot | What it checks | Viewer |
|---|---|---|
| de_dust2 T spawn | sun-lit walls, sand, des sky | warm walls, shadowed side, sky box with mountains; looks like the game |
| de_dust2 sky up/corner | sky box sides | seamless after the lf/bk swap (before: a visible seam at the top) |
| de_dust2 tunnel | dark indoor light, a lamp | dark stone, lit patch at the lamp, sunlit doorway |
| cs_office T spawn | indoor fluorescent light | as the game: grey ceiling tiles, bright light panels |
| cs_havana street | additive light cone (mode 5, 150), night sky | soft additive cone, dark sky box |
| cs_747 strobe room | style 4 `mamama` | frame means alternate 0 / 71 at 10 Hz (the room is lit only by the strobe) |
| de_aztec | opaque world water, func_water (2, 100), `{rustyrungs` (4), pred_plant model | warping water, see-through river, clean fence holes with dark edges, lit plant |

In-game screenshots: **not taken yet** (I didn't launch the user's game; steps above). The
look was checked by eye against how these maps look in CS 1.6 and by matching the engine's
maths; 66 of the 391 maps here use more than one light style (`cargo test real_look`).

## Still different / not done

- Not compared pixel for pixel with real screenshots yet (needs the steps above).
- Texgamma is applied after the GPU filters the texture (the game applies it before mipmaps);
  tiny differences in mid-tones at a distance. Lightmap planes are filtered before adding.
- `gl_max_size` / `gl_round_down` (the game shrinks textures over 256 px and rounds odd sizes
  to powers of two) not copied: big textures are sharper in the viewer.
- Detail textures (CZ `r_detailtextures 1`, `maps/<map>_detail.txt`) not drawn: CZ maps look
  plainer close up than in CZ.
- Dynamic lights, decals, sprites (`env_sprite`, glow), `renderfx` pulses, water vertex waves,
  underwater fog, animated `+0~` textures, model animation (models are in their first pose),
  model render modes, `func_` brush entities' own light origin (they use the world's lightmap
  of their faces, as in the game, but sliding doors keep their closed-position light).
- Faces without a lightmap that aren't special draw full-bright (the game: black). Rare.
- Model light uses style `m` everywhere (flickering lights don't change model light).
- The texture/model browser (agent H) keeps its own sun-lit model view; it doesn't use the
  map materials.
- World `{` textures drawn black-holed (Xash's behaviour; the game is believed to do the same:
  mappers tie fences to a mode-4 entity for that reason) — worth confirming in a screenshot.
