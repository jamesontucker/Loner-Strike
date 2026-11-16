# Studio compatibility: the rules for changing nodes and graphs

Creators' modes, library items and published community modes outlive the app version that made
them. From 1.0 on, **nothing that opens today may break**. A saved graph keeps loading, checking
and making the same plugin, whatever later versions change. This page is the rulebook for anyone
who adds or changes nodes, pins, types or the graph format.

## The compatibility suite

- **What's in it:** `app/src-tauri/src/tools/studio/testdata/compat_v3/` holds frozen copies of everything that
  shipped with graph version 3 (Loner-Strike 1.0):
  - the templates and example modes (`template-*`)
  - the library premades, Run Dialog and the Starter library (`library-*`)
  - the snippets (`snippet-*`)
  - the test graphs that use every node (`graph-every_node*`, `graph-all_nodes`, ...)
  - `graph-legacy_wires`, a hand-made graph with the wires the type system must keep working:
    - a text constant into Give Weapon
    - the hit-zone and weapon text outputs into text pins
    - As Creature's entity output into an Entity pin
    - color numbers, file paths in node settings, the transform pins
- **What it checks:** `compat_tests.rs` checks each fixture:
  1. **It loads.** It's read forward to the current graph version.
  2. **It checks with no new errors.** `out/<name>.errors` lists the errors it had in 1.0, and nothing may be added.
  3. **It generates.** The plugin must match `out/<name>.sma`, what 1.0 made.
  4. **It compiles.** It compiles with amxxpc when CrossOver is there.
  5. **It saves and reopens unchanged.**
- **Rules for the fixture files:**
  - **Never edit, regenerate or delete a fixture graph.** They are the record of what 1.0 users have.
  - Add new fixtures for later versions next to them (`compat_v4/`, ...).
  - **A different plugin from a fixture is a decision, not a snapshot refresh.** If a codegen change alters
    `out/*.sma`, read the diff. Behavior must be the same; helpers may move, and code may get faster or safer.
    Then run with `UPDATE_COMPAT=1` and say why in the commit message. `UPDATE_SNAPSHOTS` doesn't touch these.

## Rules for node authors

1. **Pin and param ids never change.** Graphs store wires by pin id and settings by param id.
   - To rename one, keep the old id in the node's alias map (`aliases` in make_nodes.py). The reader moves old wires
     and settings to the new id.
   - Labels can change freely.
2. **Node type ids never change either.** To rename a node, add the old id to the node aliases.
3. **New inputs need a default that keeps the old behavior.** An old graph has the new input unwired and unset, and
   it must make the plugin it made before. Usually that means optional, with the default doing what the node did.
4. **New outputs are fine.** Nothing in an old graph uses them.
5. **A pin's type may only widen.** Change it so every wire that was legal stays legal. Examples:
   - Entity becomes Creature on an output: it still upcasts to Entity.
   - Text becomes Weapon on an input: old text wires are kept by the `legacy_text` flag.
   - Never narrow an input in a way that refuses an old wire.
6. **A removed node isn't removed.** Mark it `deprecated: true`:
   - It stays in the registry, loads, checks and generates as before.
   - It's hidden from the palette.
   - The Problems list suggests its replacement as info, never an error.
7. **Settings keep their stored form.** A choice keeps its stored values; add new ones, don't rename old ones.
   A `path` param that becomes a typed asset still stores the same string.
8. **Graph versions only migrate forward.**
   - The reader turns versions 1, 2 and 3 into the current one in memory (`graph::migrate`).
   - Each step is additive and must not lose anything.
   - The writer writes the current version.
   - An older app opening a newer file says it was made by a newer Loner-Strike; it never guesses.
   - There is no downgrade.
9. **Generated code for an unwired, unchanged node stays the same.** The every-node snapshots and the compat suite
   both check it.
10. **Built-in enums only grow.** Add values at the end; never reorder or remove one. Saved graphs and
    Switch on Enum outputs refer to values by id.
11. **New settings get defaults that keep the old behavior, wherever they live.** That covers node params, waves.ini
    and creatures.ini keys, and mode keys. An old graph or file doesn't have the setting, and it must play as before.
    Write the setting only when it's not the default, so old files save unchanged. Example: Round Table On's
    "Creatures hunt players" (on) and "Flee when badly hurt" (Never). They're emitted only when changed, and waves.ini
    gets `hunt =` / `flee =` only when set.

## Orphaned pins and settings

Sometimes an old graph has a wire to a pin, or a value for a setting, that the node doesn't have
any more, and no alias maps it. That can happen even with the rules above, through a mistake or a
community graph from a newer build. Nothing is dropped:

- **The wire is kept.** The node shows the pin, drawn red with the label "Orphaned: <id>".
- **Problems shows an error on the node and pin:** "This pin was removed in an update. Re-wire or delete it."
  Compile, Test and Publish stop until it's fixed. Deleting the wire removes the orphan pin.
- **A removed setting's value is kept too.** Details shows it greyed with a Clear button, and Problems names it
  ("This setting was removed in an update...").
- **Saving keeps orphans as they are**, so opening the graph in a build that has the pin again restores it.
- **View > Refresh nodes** lists what an update left behind: orphaned wires and settings, and older nodes with a
  replacement. A node's pins always come from the current definitions, so there is nothing else to re-apply.

Tests: `compat_tests.rs` (`orphaned_pins_and_values_are_kept_and_named`) and `scripts/studio.test.ts`
("orphans: ...").

## Before you commit a node change

- Regenerate `nodes.json` and `nodehelp.json` with make_nodes.py and node_help.py on top of HEAD. Never commit a stale
  working copy.
- Run the Studio Rust suite. The compat suite is part of it.
- If `compat_tests` fails, fix your change. Don't refresh the outputs to make it pass.
