import { expect, test } from "bun:test";
import { join } from "node:path";

test("the page gives iOS its home-screen icon", async () => {
  const html = await Bun.file(join(import.meta.dir, "..", "index.html")).text();
  expect(html).toMatch(/<link\s+rel="apple-touch-icon"\s+href="\/apple-touch-icon\.png"\s*\/>/);
});
