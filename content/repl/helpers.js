(() => {
  const quote = (value) => `'${String(value).replaceAll("'", `'\\''`)}'`;
  const command = (strings, values) =>
    strings.reduce((text, part, at) => text + part + (at < values.length ? quote(values[at]) : ""), "");
  __define("$", (strings, ...values) => {
    if (!Array.isArray(strings) || !Array.isArray(strings.raw)) {
      throw new TypeError("$ is a template tag: write $`git status`, with values as ${...}");
    }
    return tools.Bash({ command: command(strings, values) });
  });
  __define("searchTools", (query) => tools.searchTools({ query: String(query) }));
})();
