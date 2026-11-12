export default function git(ctx) {
  const status = ctx.git.status.trim() === "" ? "(clean)" : ctx.git.status.trimEnd();
  let text = "\n# Git\nThe repository's state when the conversation started; it is not updated as you work.\n";
  text += `- Current branch: ${ctx.git.branch}\n`;
  text += `\nStatus:\n${status}\n`;
  if (ctx.git.recent.trim() === "") {
    text += "\nRecent commits: (none yet)\n";
  } else {
    text += `\nRecent commits:\n${ctx.git.recent.trimEnd()}\n`;
  }
  return text;
}
