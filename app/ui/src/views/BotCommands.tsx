import { SectionHead } from "../ui";

// What people can say to the bots (gm_botchat), its own section on the Bots page. Kept in
// step with clause_intent in plugins/scripting/gm_botchat.sma.
const GROUPS: [string, string[]][] = [
  ["Moving", ["go b", "rush a", "split a", "everyone mid", "go b slow", "fake b", "fall back", "watch flank", "follow me", "trade me", "come here", "wait here", "nvm"]],
  ["Buying", ["eco", "force", "full buy", "someone buy me an awp", "drop me an ak", "give me the bomb"]],
  ["Grenades", ["flash for me", "flash long", "smoke mid", "nade them"]],
  ["Callouts", ["two long", "awp mid", "they're all b", "bomb b", "one short he's low"]],
  ["The bomb", ["plant b", "plant here", "go defuse", "where's the bomb"]],
  ["Questions", ["money?", "what gun?", "hp?", "how many left?", "where are they?", "where are you?", "who has awp?", "are they eco?"]],
  ["Just for fun", ["jump", "crouch spam", "spray that wall"]],
];

export function BotCommands() {
  return (
    <section className="bot-commands">
      <SectionHead
        title="What you can tell the bots"
        note="Any wording close to these works, in team chat, with Bots answer you on. Start with a bot's name to ask just that one. Each bot decides by its personality: a lurker may ignore a call, a rich one may buy you that AWP, and most of the time somebody answers."
      />
      <dl>
        {GROUPS.map(([group, lines]) => (
          <div key={group}>
            <dt>{group}</dt>
            <dd>{lines.join(", ")}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
