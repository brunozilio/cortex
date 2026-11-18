(() => {
  const toolError = (tool, message) => {
    const error = new Error(message);
    error.name = "ToolError";
    error.tool = tool;
    return error;
  };
  const failed = (result) => result !== null && typeof result === "object" && "error" in result;
  const unwrap = (tool) => (result) => {
    if (failed(result)) throw toolError(tool, String(result.error));
    return result;
  };
  const quote = (value) => `'${String(value).replaceAll("'", `'\\''`)}'`;
  const command = (strings, values) =>
    strings.reduce((text, part, at) => text + part + (at < values.length ? quote(values[at]) : ""), "");
  const exit = (text) => {
    const found = /\n?\(exit code (-?\d+)\)[^\n]*$/.exec(text);
    return found ? { stdout: text.slice(0, found.index), exitCode: Number(found[1]) } : { stdout: text, exitCode: null };
  };
  __define("$", (strings, ...values) => {
    if (!Array.isArray(strings) || !Array.isArray(strings.raw)) {
      throw new TypeError("$ is a template tag: write $`git status`, with values as ${...}");
    }
    const ran = tools.Bash({ command: command(strings, values) }).then((result) =>
      failed(result) ? { ...exit(String(result.error)), failed: true } : { stdout: String(result), exitCode: 0, failed: false },
    );
    let loud;
    const thrown = () =>
      (loud ??= ran.then((result) => {
        if (!result.failed) return result.stdout;
        const how = result.exitCode === null ? "failed" : `failed with exit code ${result.exitCode}`;
        const error = toolError("Bash", `Command ${how}: ${result.stdout}`);
        error.exitCode = result.exitCode;
        error.stdout = result.stdout;
        throw error;
      }));
    return Object.freeze({
      then: (resolved, rejected) => thrown().then(resolved, rejected),
      catch: (rejected) => thrown().catch(rejected),
      finally: (settled) => thrown().finally(settled),
      text: () => thrown(),
      nothrow: () => ran.then(({ stdout, exitCode }) => ({ stdout, exitCode: exitCode ?? 1 })),
    });
  });
  __define("file", (path) => {
    const name = String(path);
    const whole = () => tools.readText({ path: name }).then(unwrap("Read"));
    return Object.freeze({
      name,
      text: whole,
      json: async () => JSON.parse(await whole()),
      lines: async () => (await whole()).split("\n"),
      exists: async () => {
        try {
          await whole();
          return true;
        } catch (error) {
          if (/No such file|does not exist|not found/i.test(String(error.message))) return false;
          throw error;
        }
      },
    });
  });
  __define("write", async (path, content) =>
    unwrap("Write")(await tools.Write({ file_path: String(path), content: String(content) })),
  );
  __define("edit", async (path, oldText, newText, options = {}) =>
    unwrap("Edit")(
      await tools.Edit({
        file_path: String(path),
        old_string: String(oldText),
        new_string: String(newText),
        replace_all: Boolean(options.all),
      }),
    ),
  );
  __define("searchTools", (query) => tools.searchTools({ query: String(query) }));
  __define("describeTool", (name) => tools.describeTool({ name: String(name) }));
})();
