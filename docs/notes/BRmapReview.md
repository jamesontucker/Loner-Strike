# br_valley v3: an independent review (0a910e7c, branch tools)

Reviewer's sources: the "br_valley v3" plan in BRmap.md, maps-src/br_valley/README.md, layout.png and overview.png,
the 16 tour views and 150 connector views, and numbers from `build_map.py`. I ran a copy of the generator in a scratch
folder; it gives a byte-identical .map, and I read its tables and placement data. I didn't render new views: the
geometry questions could be settled from the source. The black or clipped connector views (about a dozen interiors)
were checked numerically instead (factory roof hatch, barn loft ladder, the town walkways: all meet their floors with
headroom).

## Verdict: about 75% of the way to "finished and polished"

v3 does what v2 didn't: there really are three layers, every connector the validator knows about meets a floor, no
pocket is a trap, and the town's rooftop quarter is good. Five walkways all land on a doorway or a parapet gap,
with stairs, a fire ladder and drop gaps for ways down. That part is close to shippable. What keeps it from feeling
finished:

1. **The underground is big but samey and mazy, with the wrong look in places.** It's 36 identical rooms,
   1024 square and 192 high, with the same door pattern and the same furniture in each. 12 green-lit concrete
   "cisterns" make up the middle. The bunker and the cisterns share their textures. Caves have flat concrete
   ceiling strips, and the factory sits on a cave. 15 rooms have no way up of their own, and nothing points to the
   exits. It's 41% of the walkable area and 45% of the loot, and the bots don't go there.
2. **The high layer hardly grew, and it's mostly perches.** It was 6.7 M in v2 and is 6.9 M now (8% of the walkable
   area). Outside the town roofs, the factory-to-gas chain and the aqueduct, the high places are single towers
   with ladders: silo, chimney, radio tower, watchtower, barn, bunker. That's "places up high", not "high routes".
3. **The landmark structures are implausible, and the loot rewards the safest layer.** The silo is a square box
   painted with oil drums. The trees read as green drums. The aqueduct has an 800-unit span with no support. Steel
   gate frames stand alone in empty fields. Meanwhile the dark, private underground holds 24 of the 29 tier 4-5
   crates, and the surface holds none.

Counts: **must fix 6, should fix 13, nice to have 8.**

Measured loot per layer (the generator's own print): below 45 crates (avg tier 3.6, 24 of tier 4-5) and 20 guns;
surface 34 crates (avg 1.7, none of tier 4-5) and 22 guns; high 21 crates (avg 3.2, 5 of tier 4-5) and 7 guns.
There are 82 trees (the README says about 120), 30 rocks and 18 bushes.

---

## Must fix before the user sees it (looks broken or nonsensical)

**M1. Concrete ceilings over caves (the known neighbouring-ceiling issue, located).**
- Where:
  - The river cells' underside is always `TNNL_C1` (build_map.py, the river cell boxes about line 971, and the
    chamfer prisms about line 991). That puts concrete strips, in the river's S shape, over four cave rooms:
    x 128..1152, y 2432..3328 (6 cells); x 128..1152, y 1280..2304; x 1280..2304, y 1280..2304; and
    x 1280..2304, y 2432..3328.
  - Each hatch closes the room's ceiling with a 256-square `CONC` collar (lines 1229-1232). Five hatches stand in
    cave rooms: (13,20) at -384,1408; (18,23) at 896,2176; (21,23) at 1664,2176; (25,23) at 2688,2176; and
    (9,26) at -1408,2944. Conn sheets 03/04 (hatch 18-23, 21-23 and 25-23 bottoms) show rock walls under a flat
    concrete slab.
  - Every hatch shaft is `ROCK`, even in concrete rooms.
- Fix: pick the underside and the collar texture by `zone_style()` the way the terrain prisms already do (lines 870
  and 901): `ROCK` for cave and works rooms, `CEIL` otherwise. Give the shaft walls the room's wall texture.

**M2. The factory stands on a cave.**
- What's wrong: `zone_style()` makes everything west of x -2240 a cave, and the factory hall spans x -3264..-2432.
  So the factory yard's stair trench (x -3072..-2816, y -2304..-1536) is a concrete stair under a *timber mine
  portal* (the portal frame follows the zone), and it leads into a rock cave. The tour view "16 utility_room_factory"
  is in fact a cave room: rock ceiling, rock pillar, rock lump. The docs say there are utility basements under the
  factory.
- Fix: return "utility" for x < -1792 and y < -1536 too. That makes the rooms at x -3328..-2304, y -3328..-2304
  and y -2176..-1152 utility rooms, with a steel portal on the trench. The cave strip still runs up the west side
  from the fort northwards.

**M3. The silo isn't a silo.**
- Where: x -3104..-2912, y 2176..2368, 448 tall.
- What's wrong: it's a 192-square box textured `CSTRIKE_OE6TNKF`, which reads as stacked white oil drums with red
  bands (tour 12, conn 58/59).
- Fix: make it an 8- or 12-sided prism (one brush, the `blob`/`prism` helpers already do it) in corrugated metal
  or concrete, with a cap ring at the top. Keep the two ladders on flat faces and give them a cage (a few
  thin boxes).

**M4. The aqueduct's deck spans 800 units with nothing under it.**
- Where: a 24-thick deck at z 448, x 0..1344, y 2816..2944, with piers only at x 160 and x 960. That leaves an
  800-unit span over the west slope and an empty 384-unit span at the east end, before 600 units of stairs.
- What else is wrong: it's called an aqueduct but carries no water channel, and it ends in a staircase.
- Fix: add piers about every 320 (x 160, 480, 800 in the bed clear of the bed stairs, 1120). Put arches between
  them (two sloped prisms per bay are cheap) and make the deck 48 thick. Then either add a trough along one
  side, or rename it an old rail trestle or viaduct. A disused rail line ending in a platform with stairs makes
  sense.

**M5. Hatches in empty fields read as steel goalposts.**
- Where: (22,13) at 1920,-384 and (22,16) at 1920,384 (both in an open field east of the river); (22,4) at
  1920,-2688; (9,26) at -1408,2944; (13,20) at -384,1408. Also (18,23) at 896,2176 and (21,23) at 1664,2176,
  which the notes call "pump houses" though no pump house exists.
- What's wrong: each one is a 256 concrete square with a 96 hole and a free-standing two-post frame.
- Fix: put a small structure over each one that explains it:
  - a pump house on the river banks (192 square, door on the ladder side, roof);
  - a well-head or vent stack with a wooden head-frame on the mine side;
  - a brick storm-drain kiosk north of the town.
- Why it pays three ways: it makes sense, it reads from across the field, and a camper at the top can no longer
  shoot down the shaft from every side.

**M6. The trees read as green drums.**
- Where: all 82 trees.
- What's wrong: the canopy is a 7-sided prism from 48 above the ground up to 168-258, `OUT_SHRB1` on every face,
  over a 24-square trunk that's hidden under it. In every exterior view (tour 05, conn 00/01/10/11) they look like
  topiary columns or green oil tanks, not trees.
- What to keep: blocking sight from 48 up is deliberate, and it's right for gameplay.
- Fix: keep the low bottom but taper, as conifers. Use 2-3 stacked prisms, each narrower than the one below, the
  top one coming to a point, and show 16-24 of trunk under the lowest tier. Pay for the extra brushes with S7.
  Fewer, better trees beat 82 drums. A grove of 5 tiered pines blocks sight as well as 8 drums.

---

## Should fix (clear quality gains)

**S1. The underground grid: replace uniformity with places.**
- What's wrong:
  - Every room is 1024 x 1024 x 192 with two 192-wide doorways per side at ±256 off the middle.
  - Every room has the same furnishing formula: four lamps in a ±256 square, two pillars on one diagonal, the crate
    at the centre +128, a gun in every other room.
  - The middle 12 are identical concrete rooms with green lamps (`CSTRIKE_WR4CCPN` / `-0CRETE4_FLR01`, the same
    textures as the bunker's two rooms).
  - Flat 192 ceilings over a 1024 room read as a parking garage, in the caves too (tour 07, 15).
  - It is mazy: from any doorway you see a doorway.
- Fix: about 15-20 distinct spaces instead of 36 equal ones:
  - **Cisterns:** merge 2x2 rooms into one columned hall 256-320 high (floor at -576), with a water channel down
    the middle (water texture, `func_water` is optional) and two raised walkways. That's one memorable landmark
    hall in the middle instead of 12 boxes.
  - **Sewer** (x -1600..256, y 320..512): a real sewer, with a central channel 32 down and ledges.
  - **Caves:** chamfer the corners (four triangles per room), use 2-3 floor heights joined by ramps, and lower the
    ceiling in places with rock lumps. That gives the irregular silhouette caves need.
  - **Bunker:** corridors and small rooms (a guard room, an armoury as the vault, a generator room), not two halls.
  - **Utility:** machinery blocks, pipes along the walls, a boiler room under the factory.
  - **Under empty fields:** fill solid, or keep them as plain tunnels, wherever a room sits under open ground far
    from any POI (several of the east-of-river "cistern" rooms).
- What it buys: fewer, larger spaces mean fewer rib splits. The saved budget pays for the halls' extra height.

**S2. Underground wayfinding: make the way up visible.**
- What's wrong: 15 of the 36 rooms have no hatch or trench of their own, and 21 have exactly one. From inside, an
  exit looks like any other doorway, with a 120-brightness warm light at the foot.
- Fix:
  - A daylight shaft under every hatch and trench foot: a bright, slightly blue-white `light_spot` pointing down
    the shaft and a pale floor patch, so you see the way out from two rooms away.
  - A coloured stripe at hip height along the walls of the tunnels that lead to an exit, in the zone's lamp colour.
  - A stencilled sign on the wall by each doorway that leads toward a way up.
  - One ambient sound per zone, which makes "where am I" audible too: drips in the caves, hum in utility,
    a fan in the bunker, running water in the cisterns.

**S3. Loot rewards the safest layer: rebalance.**
- What's wrong: the plan says riskier is better, but the underground is the *least* exposed layer: no long
  sightlines, a private room each, many doorways to slip away through. Yet it holds 45 of 100 crates, 24 of the
  29 tier 4-5 crates, and most of the rifles. The surface has no tier 4-5 at all. In bot games (the bots go
  below 0-1% of the time) a human who goes down gets uncontested top gear.
- Fix:
  - Rooms below: tier 3 and SMGs only.
  - Tier 4-5 below only in the three vaults and the bunker core, each behind a single, noisy choke (a gate or a
    metal-grate floor you hear).
  - Put 5-6 tier 4 crates at contested surface centres: the town hall's ground floor, the factory hall floor, the
    gas station shop, the quarry shed, the sawmill and the boathouse.
  - Cut the underground to about 25 crates.

**S4. The high layer needs at least one real route, not more perches.**
- What's wrong: 6.9 M walkable (v2: 6.7 M). Apart from the town roofs, the factory roof → office → fuel line →
  gas station chain, and the aqueduct, every high place is one tower or one roof: up a ladder, then back down.
- Fix, in order of value per brush:
  1. **Extend the town's roofs east to the river.** A walkway from house F's roof (x -64, y 0, z 224) over the town
     trench's portal to a raised platform at the middle bridge's west end, then down a stair at the bank. That
     ties the rooftop quarter to the river crossing.
  2. **A ridge trail along the east ridge** (hills at 2050,-1250 / 2350,-1500 / 2050,-1850). It's already high
     ground between the quarry and the bunker. Add a plank catwalk where the crest dips, a lookout hut at its
     top, and a way down at each end.
  3. **The old rail trestle** (the reworked aqueduct, M4), continued past its stair to the lookout mesa's north
     stairs, so the north becomes a radio hill → river → lookout high line.

**S5. Perches you can't fight your way onto.**
- Where:
  - The silo (z 480), the chimney (496) and the radio tower (848) are reached only by two ladders each. The
    watchtower has a ladder plus its stairs from the sawmill roof.
  - A defender crouched behind a 40-high railing is hidden from below, and anyone on the ladder can't fight back.
  - The radio tower's top has a **tier 5** crate and overlooks the north bridge and the aqueduct.
  - The town hall roof (z 544) is 160-320 above every other roof in the quarter, so all five walkways are open
    to it.
- Fix:
  - Make one way up each tower a stair (the radio tower's lattice can hold a switchback). Where that costs too
    much, add a half-way landing with a cage, so a climber can stop and shoot.
  - Drop the tower's crate to tier 4.
  - On the town roofs, add cover the walkway users can duck behind: a water tank on house A, stair housings and
    chimney stacks on houses D and F and on the store.

**S6. The ways down don't read from across the field.**
- What's wrong: each portal is a frame 144 high, and its lamp is a 16 x 8 `~SPOTYELLOW` brush with a light of
  80-90. In daylight you can't see either beyond about 600 units (conn 02-04 tops).
- Fix:
  - A timber head-frame for the mine adit (farm trench, x -2560..-2304, y 1792..2816), about 384 tall. It's the
    classic mine landmark.
  - A stair housing with a roof over each concrete trench's top.
  - The pump houses and kiosks of M5 over the hatches.
  - Mark trenches and hatches on the overview radar.

**S7. Buy the brush budget back (clipnodes 91.9%) before adding anything.**
- **Stairs.** Every 16 of rise is its own clipping step, and stairs are everywhere:
  - 6 trenches of 36-42 steps;
  - 10 river-bed flights of 10 steps;
  - the aqueduct's 25 steps;
  - the lookout's, the fort's and the buildings' stairs.

  Put one `CLIP` ramp over each flight and make the steps non-clipping (`zhlt_noclip`, or a separate detail
  entity). Movement gets smoother and hundreds of brushes leave the clip hulls.
- **Rails.** One sloped prism per side instead of one box per step (aqueduct stairs: 50 boxes; the lookout
  stairs: one per step). Trench rails come in 128 pieces; use one box per side.
- **Trees.** Before reshaping them (M6), compile once without trees to measure their real share of clipnodes.
  If it's large, use a box `CLIP` collision for each canopy and keep the visible tiers non-solid. That isn't free:
  bots and bullets would see and shoot through non-solid foliage. Weigh it against the bots' value of cover.

**S8. Sawtooth railings and floating-looking cliff stairs.**
- Where: the lookout's cliff stairs (from x 2096, y 1536..1632, tour 13 and conn 71) and the aqueduct stairs
  (x 1344..1944).
- What's wrong: each step has its own rail block, so the rail is a staircase of 8-wide boards. At the lookout the
  rock-sided steps stuck to the cliff read as floating shelves.
- Fix: one sloped rail per side on posts; under the lookout stairs, a rock buttress down to the ground every
  128 or so.

**S9. The store roof's gap is 24 narrower than the walkway that reaches it.**
- What's wrong: the hall-store walkway is y 944..1040 (96 wide, matching the hall's 2nd-floor door at y 992),
  but the store's west roof gap is `("w", 980, 72)` = y 944..1016. That leaves a 24 ledge between the walkway's
  north rail and the parapet.
- Fix: `("w", 992, 96)`.

**S10. The east half is thin.**
- What's wrong: east of the river there are 4 places (quarry, bunker, lumber camp, lookout) against 7 in the west.
  The fields between them (x 1300..2300, y -1000..1000 and around) are open grass with few trees: long crossings
  with no cover as the zone pulls people east.
- Fix: one small place at about x 1700..2100, y -500..500, where hatches (22,13) and (22,16) already are. A river
  pumping station (a pump house, a water tower with a ladder, pipe runs as cover) would explain both hatches and
  the "cistern" rooms under that field.

**S11. Names that don't match places.**
- What's wrong:
  - The "deep chamber under radio hill" vault is the room at x -1024..0, y 1280..2304, south of the hill (the
    hill's pad is y 2560..3072).
  - The "town's cisterns" include rooms east of the river (x 1280..2304), far from the town.
- Fix: move the vault's tier 5 into the cavern_mid room (x -768..-128, y 1920..2560, which is under the hill's
  south flank), or rename it. Give the east-of-river rooms a pumping-works theme (S10).

**S12. Quarry "rock piles" are cubes.**
- Where: four `ROCK2` boxes in the pit, 96-160 on a side, at about 2432,-448; 3136,-416; 3136,192 and 2688,0.
- Fix: use `rock()` (irregular blobs with tilted tops), as the boulders do. Add a spoil heap against the pit's
  rim, which also gives the pit a second, informal way up.

**S13. The docs are stale.**
- What's wrong:
  - The README's Layout section still describes v2: "one ramp each" for the mesas, "a watchtower with a ladder",
    "~120 trees" (there are 82), "a lattice tower with a ladder" (two now).
  - BRmap's v3 section says "utility basements under the factory" (they're caves, M2).
- Fix: update both when M2 and S1 land.

---

## Nice to have

- **N1. The perimeter cliffs.** One rock texture with a regular wave silhouette on all four sides dominates every
  high view (tour 01-05, 13, 14) and gives no compass cue. Give each side a feature:
  - north: the river's gorge where it enters (a waterfall brush, or a dam wall);
  - east: a terraced quarry face;
  - south: a road cut ending in a boarded tunnel mouth;
  - west: farm terraces.
- **N2. Grass scale.** The grass at scale 2 is a coarse blur at eye level in every connector view. If AllocBlock
  (87.5%) frees up, use scale 1 on the cells of the main places.
- **N3. Props.** The "cars" in the town (four 160 x 80 x 56 rusty boxes) and the mine carts read as boxes. Add a
  cab box and four wheel stubs, or call them crates. The hay bales use a sand texture (`OUT_SND2`); a straw or
  wood texture would read as hay.
- **N4. Long ladders.** The hatch ladders are long: 512-640 units in a 96 shaft (about 3 s, helpless). A grenade
  down the shaft is an unanswerable kill. Use a 128 shaft with a landing half-way on the longer ones (25,23 at
  640; 9,26 at 600), or turn one or two hatches into short stair flights.
- **N5. Lamps.** The underground lamps are symmetric in every room. Dimmer rooms with bright exits (S2) would
  guide better than even lighting.
- **N6. Drop-in spots.** All 28 are on the ground, so the high loot (snipers, tier 3-4) is only ever reached by
  climbing. Put 3-4 drop spots *next to* a high place's way up (the hall's fire ladder, the factory's north
  ladder, the aqueduct stairs' foot), so landing near the high layer is a real choice.
- **N7. Re-shoot the clipped views.** Redo the dozen black or wall-only connector shots with the camera pulled
  inside (for example 05 bedstairs-25 bottom, 29/30 bld -1664,640, 46 factory roof hatch bottom, 52 barn loft
  bottom, 54 poi-farm bottom, 55/56 bld -2272,2560), so the next review sees everything.
- **N8. Underground through the zone.** As the zone shrinks, a player below can't see the wall coming. Make sure
  the HUD cue (or a sound) reaches players underground. Otherwise "the zone can catch you below" is a hidden
  rule, not a risk.

---

## Scale: is 1.69x enough for 20 players?

Yes. Area isn't the problem; what fills it is.
- **The size.** 86.9 M walkable is about 4.3 M per player at the start, and corner to corner is a 41 s walk. That's
  plenty for 20, and the measured games run 6-10 min.
- **Where it falls short.**
  - About 41% of the area is the underground, and the bots barely use it.
  - Only 8% is high, and most of that is tower tops.
- **What chasing 2-3x would add.** More of the same uniform space, and denser emptiness.

The trades I'd make, roughly brush-neutral:
1. **Underground: fewer, better spaces (S1).** Merge the 12 cisterns into 3 halls, give the caves irregular
   multi-height chambers, make the bunker a corridor complex, and fill the rooms under empty fields. The area
   below drops (perhaps to 25 M), but every space becomes a place with a name.
2. **Clip-hull savings (S7).** CLIP ramps over stairs and single-prism rails buy back a few percent of clipnodes.
3. **Spend the savings on height.**
   - Real high routes (S4), each adding 1-2 M of high ground that *connects*.
   - Taller underground halls, which add depth.
   - A partial second deck only where it means something: the mine (a lower stope at -704 under the deep chamber,
     reached by a ramp) and the bunker (a lower level for the core).

   That beats a generic second deck.
4. **Trees: fewer and better** (M6 + S7). About 60 tiered pines in deliberate groves block sight better than 82
   drums, and free budget.

A 1.6-1.8x valley that is legible and fair in all three layers will play better than a 2.2x one in which 40% of
the area is 36 interchangeable rooms.
