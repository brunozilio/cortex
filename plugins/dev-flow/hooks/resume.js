export const event = "SessionStart";
export const timeout = 15;

export default function resume(input, ctx) {
  let names = [];
  try {
    const listed = ctx.exec("ls .flow", { timeout: 5 });
    names = listed.code === 0 ? listed.stdout.split("\n") : [];
  } catch (error) {
    return undefined;
  }
  const open = [];
  for (const name of names) {
    if (!/^[A-Z][A-Z0-9]+-\d+\.json$/.test(name)) continue;
    try {
      const state = JSON.parse(ctx.readFile(`${input.cwd}/.flow/${name}`));
      if (state.stage !== "done") open.push(`${state.issue} is at stage "${state.stage}"${state.title ? ` (${state.title})` : ""}`);
    } catch (error) {
      continue;
    }
  }
  if (open.length === 0) return undefined;
  return {
    hookSpecificOutput: {
      additionalContext: `Development flows open in .flow/: ${open.join("; ")}. Load the dev-flow:flow skill to go on with one.`,
    },
  };
}
