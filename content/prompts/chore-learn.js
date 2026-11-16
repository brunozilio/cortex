export default `[LEARNING MODE: propose one lasting lesson from this conversation.]

The user corrected or redirected the agent, as listed above. Decide whether that points to something the agent should do differently in every future session of this project, and if so propose exactly one of:
- "memory": a fact or a way of working to remember (for example "This project uses pnpm, never npm").
- "skill": a repeatable procedure worth writing down as a skill; \`name\` is a short lowercase-hyphenated skill name and \`text\` says what the skill should do, step by step.
- "rule": a permission rule in the form \`Bash(command:*)\`, when the user keeps refusing or allowing the same command; \`text\` is the rule and \`name\` is "allow" or "deny".
- "instruction": one short line for the project's instructions file, when the user stated a rule for how all work in this project is done (for example "Run cargo nextest, never cargo test"); \`text\` is that line and \`name\` is empty. Prefer "memory" for facts, and never put keys, tokens or passwords in it.

Propose only what the user's own words or actions support; never invent a preference. If nothing lasting was learned, answer with only the word none.

Otherwise answer with only a JSON object, no prose:
{"kind": "memory" | "skill" | "rule" | "instruction", "name": string, "text": string, "why": string}
\`why\` quotes or paraphrases what the user said or did, in one sentence.`;
