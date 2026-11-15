# br_valley v5: third independent review (446ad2f6, branch tools)

What I used:
- BRmapReview.md and BRmapReview2.md, the v5 section of BRmap.md, maps-src/br_valley/README.md, layout5.png;
- all 45 tour views (12 at ground level), the 179 connector views and the contact sheets;
- the committed `.map`, `layers.json` and the shipped `plugins/game/maps/br_valley.bsp`.

My own checks, with scripts in the scratch folder `rev3/` (not in the repo):
- `raylos.py`: a line-of-sight count that traces rays through every visible brush in the `.map` (terrain,
  buildings, rocks, trees, props). It doesn't use bounding boxes or the ground grid as blockers.
- `bsp.py`, `bspstats2.py` and `placelight.py`: they read the compiled BSP's faces and lightmaps, for the light
  budget and the baked brightness of each place below.
- `wallcheck.py`: which wall texture faces into each place below, from the BSP.
- `lightsolid.py` and `column.py`: light entities inside brushes, and brush columns at a point.
- `navgen.py --check` re-run on the shipped BSP into a scratch file. Its output is byte-identical to the shipped
  .nav, and it reports "182 ends checked, 0 seams, 0 not linked; ALL CHECKS PASS".

I didn't touch any map file.

## Verdict: about 87% of the way to "finished and polished" (v4: 80%)

v5 closes every must-fix from round 2:
- The river has water.
- The town vault is closed.
- The stope is a real vault.
- Every trench is a comfortable 32-36 degrees, and the factory slot is gone.
- The nav seams are fixed. I re-ran the check and got the same 0 seams, 0 unlinked.
- The east ridge trail, the drain and pumping trenches, the boarded head-frames and the lined trenches are all in.

On geometry and bot play, the map is sound.

What's left is polish and a few things that don't work the way the notes say. Ranked by how soon a player notices:

1. **The 8 cliff-ring spires read as brick chimneys** (s01, s04, s08, t08). Scale-4 `-0OUT_RK4` on a tall, thin
   frustum looks like brickwork. They copy the factory chimney, the south-west's landmark, in every direction.
   That works against "players always know where they are".
2. **12 lights sit inside brushes and give no light.** They include half the boiler house's lamps and the red lamp
   on the ridge-trail mast, which is the south-east's new landmark. The boiler house (baked mean 30) and the
   armoury (37) are the two darkest places on the map. Neither is in the v5 tour: u09 and u12 were dropped.
3. **The cistern hall and the boathouse cellar still run into each other.** They now touch along x = -128 for 512
   units, with a 64-unit ledge and no wall. Round 2 asked for exactly this ("start the cellar at x -128"), and that
   fix was wrong: it needed a wall between them.
4. **Thin walls between two places show one place's look on both sides.** This is what's left of B4:
   - the quarry workings' west wall is pump-house rust;
   - the stope's north wall is cave rock;
   - the storm drain has concrete, utility and rock walls, but no sewer wall.
5. **The valley still reads as a bowl.** Sightlines aren't the cause; the ring is. My full-geometry count says
   long sightlines were already rare in v4 (details below). What makes the map feel small is a single
   1,250-high wall:
   - It fills half of every ground-level frame.
   - It has a periodic "shark-tooth" crest.
   - It's in the same pink as the rubble texture that now covers most ridge slopes.
6. **AllocBlock is at 92% with no plan for headroom.** Two texture-scale changes free about 8 points (see below).

**Is it ready for the user's server?**
- **For playtesting: yes, now.** No leak, every navgen check passes, 20-bot games run 4.5-6.3 min with no drop-in
  deaths, and nothing above blocks play.
- **As finished and polished: not yet.** Fix M1-M4 first. That's roughly half a day in `build_map.py` and one
  recompile. After that, I'd call it ready to ship, with the should-fix list as follow-up.

Counts: **must fix 4, should fix 10, nice to have 9.**

---

## Round 2's items: status

| Item | Status | Notes |
|---|---|---|
| B1 the river is dry | **Done well** | `!WATERBLUE` 28 deep over the whole bed, four running-water sounds (t01, conn 00-09). It reads as a river now. The fords stand proud of the water as gravel causeways, which is acceptable. |
| B2 town vault open | **Done well** | The drain moved to x -576..-384, leaving a 64 rock wall, and the only way in is `vault_way` (u03). The vault's wall into the drain shows concrete on the drain side (S1). |
| B3 stope open pit | **Done** | Out of the chamber's footprint, with one 96-wide grate way and steps (u05, u06, conn 86). It's lit, but dim: baked mean 63. The north wall is cave rock in a concrete vault (S1). |
| B4 walls below | **Mostly done** | The big mismatches are gone (armoury, guard room, sewer, town vault). Every thin wall between two different places still takes one look for both sides (S1). |
| B5 trench slopes / slot | **Done well** | Trench connectors are 32.5-35.6 degrees, landing to rim. The factory tunnel is at x -2752..-2560 at full 192 width. The bunker pad is at 64. |
| S1 sightlines | **Done, and the measurement is now right** | See "Sightlines" below. The designer's 17% / 6% is right; I get 14.6% / 5.3% tracing every brush. Round 2's 47% / 24% was a broken count. |
| S2 cliff ring | **Done poorly** | The crest is less regular, and there are notches and a waterfall. The spires backfire (M4). The waterfall is a flat olive slab (S4). The "look per side" is two textures alternating east-west and north-south, not ROCK2 on the lower third. |
| S3 two lamp grids | **Done well** | One lamp scheme per place, and the per-place colour reads (u01, u07, u18, u20). |
| S4 dark places | **Done poorly** | `lights_in()` clamps to 210, but lamps land inside pillars and boilers (M1). Baked means: boiler house 30, armoury 37, town vault 55, stope 63, against 85-120 in the other places. The two places this item named (boiler house, quarry workings) were dropped from the tour. |
| S5 cellar/cistern bleed | **Not effectively done** | The overlap became an edge-to-edge touch: a 512-long open edge with a 64 ledge (M2). |
| S6 bunker north slot | **Done** | `bunker_b` opens 192 wide into the guard room. |
| S7 east ridge trail | **Done well, except the lamp** | Catwalk, hut, mast and stairs at both ends (t05, t06, conn 82-84). The mast's red light is inside the mast (M1). |
| S8 head-frames | **Done well** | Boarded housing, door, roof, and the frame and wheel above (t03; conn 24, 25). |
| S9 trench lining | **Done** | Concrete trenches are lined (conn 10, 11, 13, 16). The adit and fort-cave trenches are rock, as they should be. |
| S10 interiors / roofs | **Mostly done** | Wood floors, tables and crates are in (conn 30, 31, 43, 55). Roofs pick one of three textures by a position hash (`build_map.py` ~1461), not by building kind, so the town still reads as one roof. Interiors are dim (conn 32-34, 40, 41). |
| S11 wayfinding | **Half done** | Stripes and an EXIT sign at both ends of every tunnel are in. Trenches and hatches still aren't marked on the radar (S7). |
| S12 trenches into the middle | **Done well** | The drain trench into the deep chamber and the pumping-works trench. Every crate below is within 15.4 s of a trench. |
| S13 re-shoots and README | **Partly done** | u13 and u18 are fixed. Conn 69-bottom is still black, and u04 (deep chamber) is now black too, although its baked light is normal (S8). The two old README claims are now true; three new ones aren't (S9). |
| N1 grass | Not done | Don't do it as round 2 wrote it: scale 1 on the grass would add about 15 AllocBlock points. |
| N2 props (cars) | Done | Cars have cabs and wheels. |
| N3 long hatch ladders | Not done | |
| N4 perches | Not done | Still two ladders up the chimney. |
| N5 pump houses | Not done | Plain boxes (s01, s04). |
| N6 viaduct | Not done | Clean concrete. |
| N7 cave ceilings | Not done | Flat 192-high rock everywhere (u07, u16, u17, u22). |
| N8 zone warning below | Not addressed | |
| N9 overview wedges | Not done | Dark wedges at the ring notches on all four edges of layout5. |

---

## Sightlines: which number is right, and does it feel big enough?

**The swapped index is real.** `build_map.py` (~line 3035) writes the ground grid row by row: `gnd[y][x]`.
Round 2's `los.py` read it as `G[i][j]` with `i` from x, so it worked on a transposed map.
- Points, viewers and terrain all transpose together, so the terrain-only figures are unaffected. That's why
  "terrain alone 52% / 29%" comes out the same in both scripts.
- The tree canopies are not transposed, so every tree stood in the wrong place relative to the hills. On top of
  that, buildings and rocks weren't counted at all.
- So round 2's 47% / 24% (v4) and the "old script 42% / 20%" (v5) are both wrong.

**My count:**
- Method: an exact ray against every visible convex brush in the `.map` (worldspawn, func_detail, func_wall and
  func_illusionary foliage). CLIP, NULL, SKY, triggers and water are skipped.
- Same setup as before: eye at 64, target at 48, points every 256 on dry ground. Points whose eye sits inside a
  brush are skipped.

| | pairs 1.5-3k visible | pairs over 3k visible |
|---|---|---|
| v4 (fd62e414), all brushes | 15.7% | 5.7% |
| **v5 (446ad2f6), all brushes** | **14.6%** | **5.3%** |
| v5, designer's corrected script (AABB of trees, rocks, buildings) | 17% | 6% |
| v5, terrain and trees, fixed index | 39% | 18% |
| v5 / v4, terrain alone | 52% / 58% | 29% / 37% |
| v5, round 2's script unchanged | 42% | 20% |

What the numbers mean:
- **The designer's 17% / 6% is right.** It's slightly pessimistic, because bounding boxes miss bushes, fences
  and props.
- **Long eye-level sightlines were already rare in v4.** Round 2's S1 was aimed at a problem the map mostly
  didn't have. v5's taller ridges, crest rocks and pine lines cut terrain-only visibility a lot (over 3k:
  37% -> 29%). But buildings and woods already blocked most of those lines, so the true figure moved by about a
  point.
- **The longest remaining views:**
  - from the east bank (x 1280..2048, y -3072..-1280) north-west across the river into the town;
  - from radio hill and the fort's flank. Those are high places and are meant to see far.

  Each sees about 100 of roughly 600 targets over 2.5k. That's healthy for a Battle Royale map.

**Does it feel big enough? Not yet, and the ring is the reason.** In the ground shots (s01, s04, s06, s08, s10,
s11):
- The play space now has three depth layers: a foreground mound, buildings in the middle, the ring behind. Hills
  hide what's behind them, which is a clear gain over v4.
- But the ring fills the top half of every frame:
  - a 1,250-high wall 1-3k away;
  - a tiled `OUT_RK3`/`-0OUT_RK4` face whose repeat is plain to see (conn 50-top, 53-top, 58-top);
  - a crest of isosceles spikes.
  
  The spikes come from the term `+ 260 if (i * 7 + j * 13) % 5 == 0` (~line 772), which is periodic, so the
  "irregular" crest still has a beat.
- The skybox's own cliffs show behind it (s01, s11) at a much bigger texture scale. That makes the ring look like
  a stage flat in front of a real horizon.
- The ridges are now mostly pink rubble, because `cell_tex()` (~line 884) turns any slope over 0.55 into
  `-0OUT_RK4` and slopes now reach 0.62. Hills and the ring are the same colour, so the eye doesn't read them as
  separate distances.

The fixes are S3 and S6: a lower, broken foreground terrace in front of the wall, and green or dirt on the
moderate slopes. That's what will make the valley feel larger, not more blockers.

---

## AllocBlock (92.2%): risk and the cheapest headroom

**My count from the shipped BSP:**
- Lit faces hold 871,669 luxels, which is 83.1% of 64 pages of 128 x 128 before packing loss. The compiler's 92%
  is that number after packing.
- Water, sky and other turbulent faces don't use lightmap space.
- The biggest users:

| texture | share | what it is |
|---|---|---|
| `OUT_RK3` | 10.4% | the ring's north and south sides, cave walls |
| `CSTRIKE_WR4CCPN` | 10.3% | concrete: places below, trench linings, housings |
| `-0OUT_RK4` | 9.9% | steep slopes, rocks, spires, cave pillars |
| `OUT_SHRB1` | 7.9% | tree foliage and bushes: 3,492 small faces at scale 1 |
| `~SPOT*` and other `~` light textures | 3.4% of the 64 pages | mostly the hip-high tunnel stripes |

**The risk:**
- Today it's low: the compile's count is the same allocator the GL client uses, and 5 pages are free.
- For anything you add, it's high. Every polish item here adds surface area. Round 2's N1 (grass at scale 1)
  alone would push the map well past 100%, and at 64 pages the client stops loading the map.
- A margin of about 15% is the comfortable place to be before a release.

**The cheapest way to free space** (texture scale changes only, estimated from the BSP faces):
1. **Foliage at scale 2** (`prism(..., "OUT_SHRB1")` ~2481, `frustum(..., "OUT_SHRB1")` ~2497, bushes ~2694:
   pass `scale=2`). That saves about **3.6 points**. Pine needles at scale 2 look the same from more than a
   metre away.
2. **The ring at the next coarser scale.** Its faces are out of reach and never seen from closer than about 200.
   Use scale 4 for the ring's cells in the ring branch of `cell_tex()`/the terrain prism (~line 936). That saves
   about **4.4 points**, and a coarser scale also tiles less visibly, which helps S3.

Together that's about 92% -> 84%. Do this before M4 and S3, which add ring geometry.

Things not worth doing for the budget:
- The stripes (3.4%): change their look instead (S10).
- Underground slab faces: the compiler already culls the faces that touch.

---

## Bot-relevant geometry

- **Re-verified:** `navgen.py --check` on the shipped BSP gives 5,255 areas, the same as the shipped .nav byte
  for byte. All checks pass:
  - 182 connector ends, 0 seams, 0 not linked;
  - 26 of 26 ladders joined;
  - 0 traps;
  - every crate below within 15.4 s of a trench.

  W2's two spots, (840, -1537) and (725, -2420), are covered by the new check.
- **Trench slopes**, from layers.json bottom to top, landing included: town, bunker and drain 35.6; farm 34.1;
  quarry 35.0; gas, factory and pumping 32.5 degrees. All have open sky over them and full-width landings.
- **Portals:**
  - The factory trench runs straight into the fort caves at full width (u19).
  - The drain trench meets `drain_n` side-on with a 256 landing.
  - The pumping trench lands straight in the works.
- **One snag at the bunker trench's foot** (S5): the guard room's sandbag barrier stands 46 units in front of the
  stairs (conn 13-bottom). You have to sidestep round it through a 64-wide gap at either end. It passes the
  checks, but it's a chicane at the main way into the bunker.
- **The cistern ledge** (M2): 512 units of one-way drop. It's not a trap, because other routes exist, but bots
  that path across it can't come back the same way.
- **The "high" area is overstated.** Of the 13,844 high samples, 7,563 are walkable from the respawn points.
  About 45% of the 8.7 M is roof and rock tops you can only land on in the drop-in. The high layer you can walk
  onto is about 4.7 M. That's fine for play, but say so in the notes (S9).

---

## Must fix

**M1. Twelve lights are inside brushes (they give no light).**
- Where (from the `.map`, light origin inside a solid brush):

| place | light origin | inside |
|---|---|---|
| boiler house | (-3072, -3136, -360), (-2560, -3136, -360) | the boilers (`CSTRIKE_ME2RSTW`) |
| ridge-trail mast, the south-east landmark's red lamp | (2376, -1300, 864) | the mast (line ~2118) |
| bunker hall | (2442, -2816, -360) | the bunker trench's mass, which runs through the hall's west end |
| cave lamps | fort caves (-3104, 176); west mine and north caves (-1856, 2624), (-1856, 3008), (-1472, 2624); ridge caves (1824, -1344); quarry workings (2656, 576) | rock pillars |
| housing lamps | factory trench housing (-2944, -2176, 360); adit head-frame (-2432, 2688, 392) | a post or beam (line ~1343) |

- Why it matters:
  - It's the main reason S4 didn't land.
  - The mast was meant to mark the south-east by its red light, and it's dark.
- Fix:
  - In `lights_in()` (~line 2188), test each lamp spot against the REG bounds of the place's props, pillars and
    trenches. Move a blocked spot to the nearest clear point at least 48 from any brush, or drop it and add one
    elsewhere.
  - Put the mast lamp 12 units off the mast's face, with a small `~LIGHT3A`-style lamp brush so it shows by day.
  - Lift the housing lamps 8 units below the roof instead of the beam line.
  - Add a validator check that refuses a light origin inside any brush. That's a few lines, using the same
    point-in-brush test as `lightsolid.py`.

**M2. The cistern hall and the boathouse cellar are still one open space.**
- Where:
  - The cisterns are x -1664..-128 at floor -576.
  - The cellar is x -128..512 at floor -512.
  - Both cover y -1280..-768.

  `column.py` at x -140 and x -116 shows open air from the two floors up to the slab top. So there's a 512-long
  open edge with a 64 ledge, and green cistern tiles and lamps meet blue cellar concrete and lamps with nothing
  between them. The BSP shows the 64-high rust ledge face at x -128 (z -544).
- Why it matters:
  - The 64 ledge is one-way for players (crouch-jump tops out below 64) and for bots.
  - Two places with their own names, lights and sounds have no edge.
- Fix:
  - Start the cellar at x -64, or end the cisterns at x -192. That leaves 64 of rock between them.
  - If you want a link, add one 96-wide flight of cistern-style steps through that wall, as the north and south
    cistern steps do.
  - Then re-shoot u21 and the cistern hall's east end.

**M3. The boiler house and the armoury are too dark, and neither was shot.**
- Where: baked lightmap mean over each place's faces, from the BSP (`placelight.py`):

| place | baked mean |
|---|---|
| boiler house | 30 (floor 39) |
| armoury | 37 |
| town vault | 55 |
| deep stope | 63 |
| fort caves | 98 |
| pumping works | 107 |
| lumber caves | 122 |

- What's wrong:
  - In the boiler house, two of its four lamps are inside the boilers (M1).
  - The armoury is 320 x 320 but 388 tall (floor -704, ceiling -316). It has one lamp at -360, 340 above the
    floor.
- Fix:
  - After M1, give the boiler house six lamps clear of the boilers.
  - In the two vaults a level down (the armoury and the stope), hang the lamps at mid-height (about -480), or add
    a second row there. Aim for a baked mean of 60 or more everywhere. Red and dim still reads as a vault at 60.
  - Add u09 (boiler house) and u12 (quarry workings) back to the tour.

**M4. The ring spires read as eight brick chimneys.**
- Where: `group("ring")`, ~line 2140. Eight frustums, r 110-140, h 1500-1800, taper 0.35, `-0OUT_RK4` at scale 4.
- What's wrong:
  - At that scale and shape, the rock texture becomes coarse blocks on a thin, straight column (s01, s04, s08,
    t08, conn 04-top, 85-top).
  - From the town you see three or four "chimneys", and only one of them is the factory's.
  - It breaks the one landmark the south-west has, and it looks like a mistake.
- Fix:
  - Make them read as rock, not masonry:
    - base r 260-380 with an irregular blob (more points, a bigger radius jitter);
    - taper 0.15-0.25;
    - lean the top 100-200 off-centre;
    - 900-1,300 tall, so they rise only 0-300 above the ring's crest;
    - `OUT_RK3` at scale 2 to match the ring.
  - Or set each one into the ring as a buttress rather than standing in front of it.
  - Five are plenty: one per side, plus one at the river's gorge.
  - Pay for it with the AllocBlock change above.

---

## Should fix

**S1. Thin walls between two places show one look on both sides (what's left of B4).**
- Where: the solid-cell merge, `near_style()` ~line 1215 and the loop ~line 1229. A solid cell between two places
  at the same distance takes the first place in `PLACES` order, and all of a block's side faces get that one
  texture. From the BSP (`wallcheck.py`):
  - The quarry workings' west wall (x 2432, y 1056..1216) shows pumping-works rust (`CSTRIKE_ME2RSTW`).
  - The ridge caves' north wall (y -1152) shows pumping-works rust.
  - The deep stope's north wall (y 2880) is cave rock in a concrete vault. You can see it in u05.
  - The guard room's west wall (x 2304) is cave rock.
  - The storm drain `drain_n`, which should look like a sewer, shows:
    - concrete on its west side (the town vault's wall);
    - `TNNL_W4` utility on its east side;
    - rock at its north end.

    It has none of its own `TNNL_W13A`.
- Fix:
  - Texture per face, not per block. When writing a solid block's side face, pick the texture from
    `style_at()` 16 units outside that face. Brushes can carry a different texture on each face, so this costs no
    brushes and no clipnodes.
  - The current merge key (the style) can then go back to `(None, None)`, which also saves the extra world
    brushes v5 added.

**S2. Free AllocBlock headroom before adding anything** (see the AllocBlock section above).
- Foliage and bushes at scale 2, and the ring at scale 4: about 92% -> 84%.
- Note in the notes that grass at scale 1 isn't affordable.

**S3. Make the ring stop reading as one wall.** Where: the ring heights, ~lines 766-777. Fix:
1. **Drop the periodic spike term**, `(i*7+j*13)%5`. Use a hash of (i, j) to raise one vertex in 6-9 at random,
   by 120-300.
2. **Add a foreground terrace on two or three sides.** Put a row of terrain cells just inside the play edge at
   300-600, lower than the wall. Where the clip wall allows, make it reachable as a ledge, or keep it out of reach
   behind the clip. That gives two distances at the edge (terrace, then wall) and the depth a valley needs. It's
   cheap: a few dozen terrain cells.
3. **Lower the wall by about 200 on the east and west**, where the skybox's cliffs show behind it. Let the skybox
   be the far horizon there.

**S4. The waterfall reads as a mossy wall.**
- Where: ~line 2142, an 8-thick `+0WATERF` slab, x 512..1280, at y PLAY+40 (t08).
- What's wrong: it's flat, olive and evenly lit, with no foam and no motion you can read.
- Fix:
  - Make it a render-only `func_conveyor` (not solid, speed 100-150, scrolling down) with a lighter, bluer
    texture.
  - Break it into 2-3 steps on the notch's rock, with a white foam strip at the foot where it meets the river.
  - Put the existing waterfall sound at the foot.

**S5. Move the bunker guard room's sandbag barrier off the trench foot.**
- Where: `P(box(2368, -2496, TUN_Z, 2624, -2464, ...))` ~line 2346. The trench foot ends at (2432, -2542), so the
  barrier stands 46 units in front of it (conn 13-bottom).
- Fix: move it to y -2400..-2368, or split it into two 96-long pieces that leave the stair line open. A barrier
  facing the stairs from 150+ away still gives the guard room its fight, without the chicane.

**S6. Slopes: less pink rubble, softer seams.**
- Where: `cell_tex()` ~line 884, `slope > 0.55 -> ROCK2`. Slopes now go up to 0.62, so most ridge faces are
  rubble.
- What's wrong:
  - Ridges and the ring read as one pink mass.
  - Straight grass/rock edges cross the slopes (s12, s08, s10).
- Fix:
  - Rock above 0.66 only. Use `GRASS2` or dirt for 0.46-0.66. Keep rock for the crest outcrops, the cuts and the
    ring.
  - Where a rock cell meets a grass cell, use a dirt cell between them.

**S7. Radar: mark the ways down, and mask the notch wedges.**
- Where: layout5 / `overviews/br_valley.bmp`.
- Fix:
  - Draw a symbol on the overview for the eight stair trenches and the twelve hatches (round 2's S11, second
    half).
  - Mask the dark wedges at the ring notches on all four edges (round 2's N9).

  Knowing where the nearest way down is, mid-zone, is the BR wayfinding that matters most.

**S8. Re-shoot the views that can't be judged:**
- u04 (deep chamber) is black. The deep chamber's baked mean is 86, the same as the fort caves (98), which shoot
  fine. Near the camera it's 74 against the fort camera's 84. So this is almost certainly the camera or the
  viewer, not the map, but check it once in game.
- Conn 69-bottom is still half black.
- s03, s07, s11 and s12 are pressed against a slope or a road and show mostly ground.
- The ground shots are at about 47 above the ground. Take a set at the standing eye of 64 as well, since that's
  what the sightline numbers use.
- Add u09 and u12 (M3).

**S9. Notes and README.** Correct or qualify:
- "every place's lamps are 210+": true of the entities, not of the result (M1, M3).
- "the boathouse cellar no longer overlaps the cisterns": true, but they're still open to each other (M2).
- "high 8.7 M": about 4.7 M is walkable from the respawn points; the rest you can only land on.
- The sightline figures: say that v4 was already about 16% / 6% with every brush counted. The v5 terrain changes
  cut terrain-only visibility; the true figure moved by about a point.

**S10. The tunnel stripes look like LED strips.**
- Where: the hip-high stripes in `~SPOTGREEN`/`~SPOTYELLOW`/`~SPOTBLUE` (u18, u20, conn 18, 20, 21, 87, 88).
- What's wrong:
  - The lamp texture's big glowing squares, 16 high along every tunnel, read as a lit marquee, not a painted
    guide line. That's garish next to the rest of the map.
  - The `~` textures cost 3.4% of the lightmap pages.
- Fix:
  - Make it an 8-high painted band in a flat colour texture (or `OUT_PAVE`/hazard yellow) at scale 2, in the
    place's colour.
  - Or use a row of small 16 x 16 lamp plates every 128, which is cheaper and reads as wall lamps.
  - Keep the EXIT signs.

**S11. Decide on the vaults against "2+ ways out of every area".**
- The town vault, the stope and the armoury each have one way in and out, by design (loot gated by risk).
- `navgen --check` counts the whole underground as one pocket with 23 ways out, so it can't see dead-end rooms.
- Either:
  - get the user to accept the vaults as the one deliberate exception, and write that in the README; or
  - give each vault a one-way exit, an up-ladder into a hatch shaft that ends at a `func_door` grate. Open it only
    from a `func_button` at the top of the ladder, so it can't be opened from outside. The grate passage stays the
    only way in, which keeps the risk.
- Either way, add a per-place "ways out" count to the check.

---

## Nice to have

- **N1. Roofs by building kind.** Choose the roof texture by kind in `building()` (~line 1461): tile for houses,
  steel for the factory and office, planks for the farm. The position hash makes the town's roofs read as one.
- **N2. Trench housings look the same.** Six of the eight are the same concrete-post canopy (conn 10, 11, 14, 16,
  17 tops). Give each a sign with the place name, or the place's material (brick in town, rust at the pumping
  station), so the way down also tells you where you are.
- **N3. Interiors are dim** (conn 32-34, 40, 41, 46). One warm light per ground floor costs nothing in
  AllocBlock, because lights aren't lightmap faces.
- **N4. Ladders below stand free in the middle of rooms** (conn 18, 22, 24, 26 bottoms). Add a 16-deep collar or
  shaft lining where the ladder meets the ceiling, so it reads as coming down a shaft.
- **N5. Pump houses** are still plain boxes (round 2's N5): add a door frame, an overhang and a valve wheel.
- **N6. Viaduct**: stone on the piers and corbels, rust on the rails (round 2's N6).
- **N7. Cave ceilings**: raise one chamber per cave to 256-320 (round 2's N7).
- **N8. The zone warning below**: check that it reaches a player 400 under the surface (round 2's N8).
- **N9. Long hatch ladders** (round 2's N3): a half-way landing in the 560-640 ladder shafts.

---

## Against the user's bar

- **Polish is the difference.**
  - Above ground, the buildings, bridges, viaduct, silo and the new trail are clean.
  - What lets it down:
    - the ring and its spires (M4, S3);
    - the waterfall (S4);
    - pink rubble on the slopes (S6);
    - the marquee stripes below (S10).
  - Below: dark rooms (M1, M3) and mixed walls (S1).
- **Every element has a purpose:** yes. The only items without one are the spires, which currently mislead.
- **Three real layers, 2+ ways out:** the layers are real. The surface is 43.9 M, below 16.9 M, and high about
  4.7 M walkable (8.7 M including the roofs you can only land on). Everything has 2+ ways out except the three
  vaults, which needs a decision (S11).
- **Feels larger than it is:** better than v4. Hills now hide what's behind them, and the river is a band to plan
  around. The ring still makes it a bowl (S3, S6).
- **Players always know where they are:**
  - Below: good. Each place has its own colour and sound, EXIT signs, stripes.
  - Above: the north is good. The south-east's mast lamp is dark (M1), the spires copy the chimney (M4), and the
    radar doesn't show the ways down (S7).
