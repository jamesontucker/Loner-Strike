# br_valley v4: second independent review (fd62e414, branch tools)

What I used:
- BRmapReview.md (the v3 review), the v4 section of BRmap.md, maps-src/br_valley/README.md, layout.png and
  overview4.png;
- all 44 tour views, the 5 retakes and the 173 connector views (contact sheets and the full-size frames where
  something looked off).

I ran a copy of `build_map.py` in a scratch folder. Its .map is byte-identical to the committed one. From that copy
I probed the tables, the slab grid and the written brushes. I also ran a terrain-and-tree line-of-sight count on
the `layers.json` ground grid for v3 (0a910e7c) and v4. I didn't render new views, and I didn't touch any map file.

## Verdict: about 80% of the way to "finished and polished" (v3: 75%)

v4 is a real step forward:
- The brush budget is back: clipnodes went from 92% to 77%.
- The silo, viaduct, trees and hatch kiosks now read as what they are.
- The level below has names, colours and sounds.
- Loot follows risk on paper.
- The town's roofs reach the river, and the viaduct gives the north a true high line.

Most of the first review's list is done, and done well. The remaining 20% is the kind the user notices in the
first two minutes:

1. **The river valley has no river.** The bed is dry mud from end to end, under docks, piers, a boathouse, two pump
   houses on the banks and a river pumping station. That's the biggest plausibility hole on the map, and the
   first review missed it too.
2. **The places below don't hold their edges, and two of the three vaults aren't vaults:**
   - The town vault's whole east side is open to the storm drain.
   - The deep stope is a pit open on three sides inside the deep chamber.
   - 35 solid blocks below show one place's wall texture to a neighbour that looks different: rock in the town
     vault, utility wall in the armoury.
   - The tunnels carry two overlapping lamp grids in different colours.

   That undoes much of S1/S2/S3's "distinct places" work.
3. **The terrain doesn't yet make the valley feel bigger.** At eye height, the ridges cut long views only a little:
   - pairs 1.5-3k apart: 53% visible in v3, 47% in v4;
   - pairs over 3k apart: 27% in v3, 24% in v4.

   That count is terrain and trees only, not buildings. A single-texture cliff ring with a regular wave silhouette
   closes every ground view, so the map reads as a bowl.

Counts: **must fix 5, should fix 13, nice to have 9.**

The numbers I checked:
- Loot as the generator prints it: below 21 crates (3 of tier 4-5) and 10 guns; surface 35 crates (6 of tier 4)
  and 23 guns; high 22 crates (2 of tier 4-5) and 7 guns.
- 68 trees, 19 rocks, 19 bushes, 14 foxholes and berms.
- Below: 18 places (15.6 M sq units of rectangles) plus 4.1 M of tunnels.

---

## The first review's items: status

| Item | Status | Notes |
|---|---|---|
| M1 concrete ceilings over caves | **Done, but the same class of bug is still in the walls** | Ceilings, bed and chamfer undersides and hatch collars now follow `zone_style()`. The walls don't: see must-fix B4. |
| M2 factory on a cave | **Done well** | The boiler house is utility. Its link north is broken by the trench, though: see must-fix B5. |
| M3 silo | **Done well** | Eight sides, cap ring, caged ladder, stair scaffold, bridge. The rust texture reads more as a rusted tank than as corrugated sheet; acceptable. |
| M4 aqueduct span | **Done well** | Piers every ~320 with corbels, a 48 deck, rails, carried on to the lookout. Clean modern concrete for an "old" line (nice-to-have N6). |
| M5 hatches in fields | **Done well for kiosks, pump houses and sheds; poorly for head-frames** | The two hatch head-frames are open frames (t03): they read as a gallows or swing set, and the shaft can still be covered from every side (S8). |
| M6 trees | **Done well** | 68 tiered pines read as pines (t12, s09). The CLIP-prism trick is sound. |
| S1 underground places | **Mostly done** | 18 places, each with its own walls, floor, light colour and sound; caves have cut corners, pillars, hanging rock and a shelf. Still flat 192 ceilings in every cave, and the cisterns have no raised walkways. Places bleed into each other: B2, B3, S5. |
| S2 underground wayfinding | **Partly done** | Daylight down every shaft and trench, an EXIT sign at each foot, a sound per place. No directional cue in the tunnels themselves (S11). |
| S3 loot by risk | **Done in counts, broken in gating** | 21 crates below, tier 5 only in three vaults, tier 4 at six contested surface places, radio tower down to 4. Two of the three vaults are open (B2, B3). |
| S4 high routes | **2 of 3 done** | Town roofs to the river and the viaduct are done; the east ridge trail isn't (S7). High area 6.9 to 8.4 M. |
| S5 perches | **Mostly done** | Silo stair, radio tower caged landing and roof cover are in. The chimney is still two ladders (N4). The roof cover is small (48 square stacks). |
| S6 ways down read | **Mostly done** | Housings and the adit's head-frame read from across the field (conn 10-15 tops). Not marked on the radar (S11). |
| S7 brush budget | **Done well** | Non-clipping steps under CLIP ramps, CLIP prisms round foliage, CLIP helpers in the detail entity: clipnodes 92 to 77%. |
| S8 rails | **Done well** | |
| S9 store roof gap | **Done** | |
| S10 east half thin | **Done** | The pumping station gives the east a place. The pump houses are plain boxes (N5), and the field round them is still open (s08). |
| S11 names | **Done** | |
| S12 quarry rocks | **Done** | Rock blobs and a spoil heap. |
| S13 docs | **Done, with two new claims that aren't true** | "No sightline runs across the valley" and "each vault behind one grate-floored way in" (S13). |
| N1 perimeter cliffs | **Not done** | Promoted to should-fix (S2): the new ground-level views show it in every frame. |
| N2 grass scale | **Not done** | |
| N3 props | **Not done** | Cars, carts and hay (`OUT_SND2`) are unchanged. |
| N4 long hatch ladders | **Not done** | Still 512-640 (less important now that trenches are the main ways down). |
| N5 lamps | **Partly done** | Zone colours are in, but the grid is still even, and the tunnels have two grids (S3). |
| N6 drop spots by high routes | **Done** | |
| N7 re-shoot clipped views | **Done for the old list; new ones broken** | u13 (bunker hall) and u18 (town trench foot) are shot from inside solid. Conn 45-bottom, 69-bottom and both ends of 83 are black (S13). |
| N8 zone cue below | **Not addressed in the notes** | N8. |

---

## Must fix before the user sees it

**B1. The river is dry.**
- Where: every river cell. The bed is at z -160, 512-1024 wide, and runs the full height of the map.
- What's wrong:
  - The only water brushes in the map are the sewer channel and the cistern channel (12 `!WATERBLUE` faces).
    The bed is cracked mud (conn 00-09 bottoms, s01).
  - Yet the map has docks with piers "over the river", a boathouse with a cellar, pump houses on both banks, a
    "river pumping station", fords and stone steps "out of the bed".
  - Even the `cliff` skybox shows a river: it's visible in u13.

  A player reads this as a dry canal at best, and as unfinished at worst.
- Fix: one `!WATERBLUE` (or `!WATERGREEN`) world brush per river row, from the bed to about 28 above it (top at
  about -132), between the bank faces. Merge rows that share x extents.
  - Why 28: under about 35 deep a standing player's origin stays dry (waterlevel 1). There's no swimming and no
    slowdown, bots path as before, and the bed stairs and fords still work, but footsteps turn into wading
    splashes. That gives you a free audio cue for anyone crossing.
  - Liquid brushes stay out of the clip hulls, so clipnodes don't move. They add some leaves and faces (worldleaves
    is 78%, so check after the compile).
  - Add 3-4 `ambient_generic`s of running water along the bed.
  - Optionally, make the fords' ramp cells 8 shallower so they read as fords.

**B2. The town vault is open along its whole east side.**
- Where: the town vault is x -1088..-640, y 640..1024. The storm drain `drain_n` is x -640..-448, y 512..1920.
  They share the line x = -640, so 384 units of the vault's east side are open floor into the drain (retake u03:
  the "vault" is a bay off a lit corridor with a ladder and an EXIT sign).
- What's wrong: the tier 5 crate is reachable from the drain without crossing the grate passage. "One
  grate-floored way in" isn't true.
- Fix: move `drain_n` to x -576..-384 (it still meets the sewer, y 320..512, and the hatch (13,20) foot room at
  x -512..-256). Or shrink the vault to x -1088..-704. Either way a 64 rock wall stands between them. Re-run the
  checks: the vault's only way in becomes `vault_way`.

**B3. The deep stope is an open pit, not a vault.**
- Where:
  - The deep stope is x -896..-384, y 2560..2944, at floor -704.
  - The deep chamber is x -1152..-128, y 1920..2816, at -512.
  - They overlap on y 2560..2816. `slab_floor` takes the lower floor, so the chamber has a 192-unit ledge along the
    stope's south edge (512 long), its west edge and its east edge (256 each).
- What's wrong:
  - Anyone can drop in anywhere along about 1000 units of edge, and no grate is involved.
  - The only way back up is the 96-wide "deep stope steps" with a sideways exit at the bottom.
  - It's also nearly black: one lamp of 150 red for 512 x 384 x 384 (u05; conn 83 both ends).
- Fix:
  - Move the stope out of the chamber's footprint: y 2816..3200, still under the hill's flank.
  - Join it to the chamber with a 96-wide grate passage like `vault_way`, leading to the steps.
  - Give it 2-3 lamps at 220+.

  It then works like the armoury, which is built right.

**B4. Walls below show the wrong place's look (review M1, in the walls).**
- Where: the slab merge in build_map.py (the loop after `near_style()`, about line 1218). Solid cells all share
  the key `(None, None)`, so the greedy merge builds big solid blocks. `near_style()` then gives each whole block
  the style of the first place it touches. 35 of the 75 underground solid blocks border places with different
  wall textures, for example:
  - The town vault's west and north wall is cave rock (block x -1792..-1088, y 640..1920). It shows in the
    retake u03.
  - The armoury's south wall is utility `TNNL_W4` (block y -3840..-3328, shared with the boiler house and the
    tank vault). Its east wall is works rock (x 3264..3840). It shows in conn 86.
  - The guard room's north wall is cave rock (x -256..2304, y -2560..-2304). The bunker hall's south wall is
    utility (x -640..2304, y -3328..-2560).
  - The sewer tunnel `river` and `docks_s` are lined with the pump house's rusted corrugation (x -800..1664,
    y -1824..-1536).
  - The town trench's tunnel junction (x 256..512, y -512..352) has sewer walls facing the pump line.
- Fix: put the style in the merge key. Before merging, compute `near_style()` per grid cell (each cell is already
  bounded by place and tunnel edges), so a block only merges with cells that face the same look. It costs a few
  dozen world brushes (clipnodes are at 77%). Then re-check u03, u10, u14 and conn 86.

**B5. The factory trench is at the engine's slope limit and squeezes the factory tunnel into a 48-wide slot. The
bunker trench is also too steep.**
- Where:
  - `trench:factory` (cells 3, 6..9) rises 512 over a 512 run: **45.0 degrees**, 32 steps with 16 treads. The
    engine's walkable limit is a normal z of 0.7, about 45.6 degrees, so the CLIP ramp clears it by about 1%.
  - `trench:bunker` is 672 over 768: **41.2 degrees**.
  - Every other way down is 33.7-38.7 degrees (computed from the connectors in layers.json).
- The second problem: the factory tunnel (x -2944..-2752) runs through the trench's cells. The trench's east side
  wall at x -2816..-2800 leaves the tunnel's remainder as a **48-wide, 512-long, 192-high slot**, from x -2800 to
  -2752 and y -2048 to -1536. That slot is the only floor-level way from the boiler house north to the fort caves.
  Bots path through it hull-to-wall; a player sees a crack.
- Fix:
  - Make the factory trench 4 cells (cells 3, 5..9), or start the stairs at the landing's far side. That gives
    768 of run, 33.7 degrees.
  - Move the `factory` tunnel to x -2816..-2624, so it meets the trench's landing side-on and runs past its east
    wall with full width. Check its meeting with `gas_w` (y -2016..-1824).
  - Lengthen the bunker trench by one cell (cells 24, 1..6: 1024 of run, 33.3 degrees). Or lower the bunker pad
    to 64, which also helps its approach slope.
  - Re-run `navgen --check` (the walk from each trench to each crate).

---

## Should fix (clear quality gains)

**S1. Ridges and lanes are too gentle to break sightlines.**
- What's wrong:
  - The six ridges are 160-230 high on 300-380 half widths; the three lanes are 72-88 deep.
  - My line-of-sight count: eye 64 over the ground grid, targets at 48, every 256 on dry ground, with tree
    canopies as blockers. Buildings aren't counted.
    - Pairs 1.5-3k apart: 53% visible in v3, 47% in v4.
    - Pairs over 3k: 27% in v3, 24% in v4.
  - The ground shots agree: s01 sees across the river to the east cliff, and s08 sees from the east field to the
    town. The README's "no sightline runs across the valley" isn't true.
- Fix, roughly in order of effect per brush:
  1. **Crest each ridge with rock outcrops.** Put 2-3 `rock()` blobs, 160-260 tall, along each ridge's crest line,
     40+ apart as the validator wants. Add a short pine line (3-5 tiered pines) on the crest. A crest that's 200
     high plus 200 of rock and trees blocks like a 400 ridge, at about 15 brushes a ridge.
  2. **Add two ridges where the views are longest:**
     - the east field between the pumping station and the quarry (around x 2000..2400, y -1000..900);
     - the farm-to-town gap (x -2200..-1500, y 1500..2000).
  3. **Deepen the lanes to 120-140** where the slope clamp allows (the gas station-docks lane first). A crouched
     player in a 128 lane is out of sight; at 88 they aren't.
  4. **Set a target and re-run the count:** under 12% of the over-3k pairs visible, and under 35% of the
     1.5-3k pairs.

**S2. The cliff ring is one texture with a sine-wave crest (review N1, promoted).**
- Where: the backdrop ring, about line 758.
- What's wrong:
  - Every ground-level frame (s01-s12) is half `OUT_RK3`, tiled at the same scale, with a regular wave on all
    four sides.
  - It gives no compass cue and makes the valley read as a bowl. The skybox's own cliffs (u13) show what it
    could look like.
- Fix:
  - Mix 2-3 harmonics with different phases per side, so the crest is irregular, and put a few tall spires in
    front of it.
  - Use `ROCK2` for the lower third.
  - Give each side a feature:
    - north: a gorge where the river (B1) enters, with a waterfall brush;
    - south: the road cut and a boarded tunnel mouth where the south road leaves;
    - east: the quarry's terraced face;
    - west: West Peak's shoulder.

**S3. The tunnels have two lamp grids.**
- Where:
  - build_map.py, about line 1403: "tunnel lights every ~384". A warm `255 214 150 160` light with a `~LIGHT3A`
    brush, in every tunnel, including the stretches that run through places (the mine gallery through the deep
    chamber, the river tunnel through the ridge caves, the old sewer through the cisterns).
  - About line 2190: "lighting along the tunnels" adds the zone-coloured set every 448, and skips the places.
- What's wrong: two overlapping rhythms, and warm white over green and blue. That blurs the per-place light that
  is the main "where am I" cue below.
- Fix: delete the first loop. That also saves its detail brushes.

**S4. Some places below are too dark to read.**
- Where:
  - The boiler house (u09: black, with the boilers as silhouettes).
  - The stope (B3).
  - The quarry workings' far end (u12 is dim).
- Fix:
  - At least 200 for every place's lamps, and a lamp every 256 in rooms under 600 across.
  - Or turn on texture lights for the lamp textures in RAD, so the fixtures glow.

  Dark is fine for mood, but a player has to see a shape at 400 units.

**S5. The boathouse cellar and the cistern hall bleed into each other.**
- Where: the cellar is x -256..512, y -1536..-768 at -512; the cisterns are x -1664..-128, y -1280..-128 at -576.
  They overlap on x -256..-128, y -1280..-768.
- What's wrong:
  - The cistern floor runs 128 into the cellar, with a **64 ledge** along x = -128 and y = -1280. Only a
    crouch-jump gets up it, so it's one-way for most players and awkward for bots.
  - The two places' textures and lights meet with no wall.
- Fix: start the cellar at x -128 (or end the cisterns at x -256). If you want a link, put cistern-style steps
  in it, like the north and south cistern steps.

**S6. The bunker's north tunnel meets the guard room through a 64-wide slot.**
- Where: `bunker_b` (x 2688..2944, y -2368..-1824) touches the guard room (x 2304..2688, y -2688..-2304) only at
  the corner x 2688, y -2368..-2304.
- Fix: widen the overlap to 192 (start `bunker_b` at y -2496), or close it so the tunnel enters by the generator
  room alone. A 64 slot is an odd spot for a fight.

**S7. No high route in the south-east, and no landmark there (review S4.2).**
- What's wrong:
  - The bunker and quarry quarter has a roof and one ladder: no route.
  - The south-east has no tall silhouette to steer by, unlike the north (radio tower, viaduct, silo, head-frame)
    and the south-west (chimney).
- Fix: the east ridge trail:
  - a plank catwalk along the east ridge (hills at 2050,-1250, 2350,-1500 and 2050,-1850), from the quarry rim to
    the bunker yard;
  - a lookout hut at its top, with a short mast, as the quarter's landmark;
  - stairs at both ends.

  That's 1-1.5 M of high ground that connects, in the quadrant that has none.

**S8. The hatch head-frames read as gallows.**
- Where: hatches (9,26) at -1408,2944 and (25,23) at 2688,2176 (t03; conn 24 top).
- What's wrong: four bare posts and a beam over a 40-high collar. From the field it's a swing set, and the shaft
  is open to a camper on every side, which is what M5 meant to stop.
- Fix: board the lower 112 on three sides with `OUT_WD` and put a door on the ladder side, like the sheds. Add a
  pitched plank roof at about 160, under the beams and wheel. It still reads as a head-frame, because the tall
  posts and the wheel stay.

**S9. The concrete trenches are cut into raw rock, and their feet have floor seams.**
- Where: the town, gas station, factory, bunker and quarry trenches (conn 10-14, both ends).
- What's wrong:
  - Concrete stairs under a concrete housing run down between pink rock faces.
  - The factory trench's foot is half mud and half concrete (conn 12 bottom).
  - u18 shows the rock running right down to the tunnel lintel.
- Fix:
  - Face the trench's above-slab walls with `CONC` (or the place's wall texture) where `zone_style` isn't cave.
    The adit's rock is right.
  - Give the landing the floor of the place it opens into.
  - Add a 16 concrete lip at the top edge.

**S10. Building interiors are all steel plate and empty, and every roof is the same.**
- Where: `building()` defaults to `floor_tex="CSTRIKE_FE2SQRS"` (riveted steel) for every house, the hall, the
  farmhouse and the boathouse (conn 30-39, 48, 56). The roofs share `OUT_ROOF2B` and the green grid.
- Fix:
  - Floors: wood planks for houses, the farmhouse and the boathouse (`CSTRIKE_FJ2DOCK` or `OUT_WD`), stone or tile
    for the hall, steel only for the factory and the office.
  - Two or three props per ground floor (a table box, a shelf, a crate) as cover.
  - Two or three roof textures, picked by building kind.

  It's cheap, and it's what makes "polished" visible inside.

**S11. Wayfinding: cues between the exits.**
- What's wrong: the daylight and EXIT signs work once you're at a foot (u17). In between, a tunnel junction gives
  no hint which way the nearest stair trench is, and the radar doesn't show trenches or hatches.
- Fix:
  - A 16-high painted stripe at hip height (a thin `func_illusionary` in the zone's lamp texture or a yellow
    hazard texture) on the walls of the tunnels that lead to a trench, with an EXIT sign at each junction facing
    the right way.
  - Draw the six trench housings and fourteen hatch huts on the overview with a mark (the radar maker's .txt
    layer, or a painted symbol on the .bmp).

**S12. Give the middle below a stair trench; bots use only trenches.**
- What's wrong: the deep chamber (the vault's host, under radio hill) has no way up of its own. The pumping works
  have only two ladders. Bots don't climb ladders down, and the brain drops its orders after 20-40 s, so places
  more than about 15 s from a trench stay empty.
- Fix:
  - A stair trench on radio hill's south flank (down from the north road at about x -700, y 1700..2600) into the
    deep chamber, under a head-frame (it's the mine's).
  - Convert one pumping-station hatch, (22,16), into a short trench under a pump-house housing.

  Both put ways below where the surface traffic is, for people and bots alike.

**S13. Re-shoot the broken views and correct two README claims.**
- Views:
  - u13 "bunker hall" and u18 "town trench foot" are shot from inside solid: you see the skybox and back faces.
  - Conn 45 bottom, 69 bottom and both ends of 83 are black.

  Without them the bunker below can't be judged by eye.
- README and BRmap:
  - "no sightline runs across the valley" (fix with S1 or reword);
  - "each behind one grate-floored way in" (true only after B2 and B3).

---

## Nice to have

- **N1. Grass.**
  - `CSTRIKE_FT3GRAS` at scale 2 is a coarse blur at eye level in every frame.
  - Patches of `-0OUT_GRSS1` at another scale make visible seams (s12).
  - Fix: scale 1 on the main places' cells if AllocBlock (84%) allows. One grass texture per patch.
- **N2. Props.** The cars are boxes, the mine carts are boxes, and the hay is `OUT_SND2` (review N3). Add a cab box
  and wheel stubs to the cars, and use a straw or wood texture for the hay.
- **N3. Long hatch ladders** (review N4). (25,23) is 640 and (9,26) is 560. A half-way landing in a 128 shaft would
  help.
- **N4. Perches.**
  - Give the chimney a caged half-way landing like the radio tower's.
  - Make the roof cover on the hall-facing roofs bigger: the 48-square stacks hide a crouched player's head only.
- **N5. The pump houses are plain concrete boxes** (s01, s08). Add a door frame, an overhanging roof, a window and a
  valve wheel on the pipe, so they read as pump houses from across the river.
- **N6. The viaduct is clean modern concrete** for an "old rail" line. Use a stone or brick texture on the piers and
  corbels (`OUT_WALL3A` or a stone texture), rust on the rails, and a few missing parapet pieces.
- **N7. Caves: vary the ceiling.** Every cave is 192 high with a flat rock ceiling (u04, u06, u16, u20). Raise one
  chamber per cave to 256-320, the way the cisterns are. Give the cisterns the two raised walkways from the first
  review.
- **N8. The zone below.** Check that the shrinking zone's warning (HUD or sound) reaches a player 400 under the
  surface. It isn't mentioned in the v4 notes.
- **N9. The overview** has two dark wedges outside the play area: the top edge near x 0, and the west edge near
  y -2200 (overview4). These are probably the out-of-reach buy zone and backdrop pieces. Mask them in the radar
  render.

---

## Scale: is 1.35x still enough for 20 players?

**By area, yes. By feel, not yet.**
- **The numbers.** 69.6 M walkable is 3.5 M per player at the start, and the surface alone is 2.2 M per player.
  Corner to corner is still 41 s.
  - The bot games ran 4-5 min because 7-12 of 20 bots were armed at the fight start. With people, expect 6-9 min.
  - The area lost below was 36 interchangeable rooms. The 18 named places hold attention better than the old
    35.9 M did, and the bots barely used either.
- **Why it still feels smaller than it is:**
  - long views across the middle (S1);
  - one bowl of cliffs (S2);
  - a dry bed that is just another field to cross (B1).

**The most effective additions, by effect per brush (clipnodes 77%, AllocBlock 84%, worldleaves 78%):**
1. **Water in the river (B1).** It costs no clipnodes. Bridges, fords and the bed stairs become real crossing
   choices, and every crossing makes noise. The river becomes a band to plan around, which is what makes a valley
   feel wide.
2. **Rock-and-pine crests on the ridges, and two new ridges (S1).** Each hidden quarter feels like a new place
   when you crest into it. That's the cheapest "bigger".
3. **The east ridge trail (S7).** About 1-1.5 M of connected high ground, and a landmark in the one quadrant that
   has neither.
4. **Two more stair trenches into the middle below (S12).** That doesn't add area; it makes the area that exists
   (16.8 M) reachable for bots and inviting for people.
5. **Don't add more area below** until B2-B4 and S3-S5 make the 18 places read as 18 places.

---

## Bot-relevant geometry: are the stair trenches bot-friendly?

**Mostly yes. The geometry isn't what keeps bots on top, with two exceptions: the factory trench and its slot.**
I agree with the designer that the bots dropping their orders after 20-40 s is the brain's doing.

- **Slopes.** Computed from the 48 stair connectors (rise over run, ramp to ramp):
  - Gas station 33.7, farm 35.3, town 36.9 and quarry 36.9 degrees: fine.
  - Bunker 41.2: steep but walkable.
  - **Factory 45.0, at the engine's limit (B5).**
  - Everything else (building stairs, bed steps, the viaduct, the silo, the cistern and vault steps) is 29.7-38.7.
  - The walking surface is the CLIP ramp, so there's no step-climbing for bots. It starts at the landing's floor
    and ends at the pad's height, with no lip.
- **Headroom.** Every trench is open to the sky. The concrete housings clear 160 over the top 256; the adit
  head-frame's lowest beam is at +192. Each landing is 256 deep, and its opening into the place or tunnel is the
  full 192 tunnel height. Nothing is under 128.
- **Turns at the portals:**
  - Town: the landing turns 90 degrees into the sewer (192 wide) or the pump line (128 wide), with the 256 landing
    to turn on.
  - Gas station: 90 degrees into the tank vault or the 192 `gas_s`.
  - Bunker: straight into the guard room.
  - Quarry and farm: straight on.
  - **Factory:** straight into a 128-wide overlap, and then the only way north is the 48 slot (B5).

  The narrow passages are the vaults' 96-wide grate ways and steps, which bots don't need.
- **Hatches.** The ladders are on the north wall. You climb out into a 56-deep alcove and leave by an 80 x 112
  door straight ahead. That's fine for people; bots won't use ladders down anyway. The trenches carry the bots.
