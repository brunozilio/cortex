(() => {
  const begin = globalThis.__begin;
  const log = globalThis.__log;
  delete globalThis.__begin;
  delete globalThis.__log;
  const pending = new Map();
  const ring = [];
  const RING = 1000;
  const owners = () => {
    const found = [];
    for (const match of String(new Error().stack).matchAll(/\(script-(\d+):/g)) found.push(Number(match[1]));
    return JSON.stringify(found);
  };
  const fixed = (name, value) =>
    Object.defineProperty(globalThis, name, { value, writable: false, enumerable: false, configurable: false });
  fixed("__define", fixed);
  const named = (value) => `[function ${value.name || "anonymous"}]`;
  const show = (value) => {
    if (value === undefined) return "undefined";
    if (typeof value === "string") return value;
    if (typeof value === "function") return named(value);
    if (value instanceof Error) return `${value.name}: ${value.message}`;
    try {
      const text = JSON.stringify(
        value,
        (_, item) => (typeof item === "bigint" ? `${item}n` : typeof item === "function" ? named(item) : item),
        2,
      );
      return text === undefined ? String(value) : text;
    } catch {
      return String(value);
    }
  };
  const failure = (reason) => {
    if (reason instanceof Error) {
      const where = reason.name === "ToolError" ? "" : String(reason.stack || "").trim();
      return where === "" ? `${reason.name}: ${reason.message}` : `${reason.name}: ${reason.message}\n${where}`;
    }
    return `Thrown: ${show(reason)}`;
  };
  const call = (name, input) =>
    new Promise((resolve, reject) => {
      const id = begin(String(name), JSON.stringify(input === undefined ? {} : input), owners());
      pending.set(id, { resolve, reject, name: String(name) });
    });
  const identifier = /^[A-Za-z_$][\w$]*$/;
  const plain = (value) =>
    value !== null && typeof value === "object" && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype;
  fixed("__settle", (id, ok, body) => {
    const waiting = pending.get(id);
    if (waiting === undefined) return;
    pending.delete(id);
    if (ok) {
      waiting.resolve(JSON.parse(body));
      return;
    }
    const error = new Error(body);
    error.name = "ToolError";
    error.tool = waiting.name;
    waiting.reject(error);
  });
  fixed("__show", show);
  fixed("__failure", failure);
  fixed("__finish", async (value) => {
    const settled = await value;
    if (!plain(settled)) return settled;
    const entries = await Promise.all(Object.entries(settled).map(async ([key, item]) => [key, await item]));
    return Object.fromEntries(entries);
  });
  fixed("__expose", (names) => {
    for (const name of names) {
      if (identifier.test(name) && !(name in globalThis)) fixed(name, (input) => call(name, input));
    }
  });
  fixed("tools", new Proxy(Object.freeze({}), { get: (_, name) => (typeof name === "string" ? (input) => call(name, input) : undefined) }));
  fixed("self", Object.freeze({ log: (...parts) => void log(owners(), parts.map(show).join(" ")) }));
  const remember = (...parts) => {
    ring.push(parts.map(show).join(" "));
    if (ring.length > RING) ring.splice(0, ring.length - RING);
  };
  fixed(
    "console",
    Object.freeze({
      log: remember,
      info: remember,
      warn: remember,
      error: remember,
      debug: remember,
      tail: (count = 20) => ring.slice(-count).join("\n"),
    }),
  );
  fixed("preview", (value, length = 1000) => {
    const text = show(value);
    if (text.length <= length) return text;
    return `${text.slice(0, length)}\n[${text.length} characters in all; the last ${text.length - length} are not shown]`;
  });
})();
