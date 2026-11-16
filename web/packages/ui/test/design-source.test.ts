import { describe, expect, test } from "bun:test";

describe("design source", () => {
  test("component styles never use the agent color outside agent identity", async () => {
    const css = await Bun.file(new URL("../src/styles/components.css", import.meta.url).pathname).text();
    const rules = css.split("}").filter((r) => r.includes("var(--agent)"));
    const selectors = rules.map((r) => (r.split("{")[0] ?? "").replace(/\/\*[\s\S]*?\*\//g, "").trim());
    for (const selector of selectors) {
      expect(selector).toMatch(/ac-avatar|ac-logo|ac-msg--streaming|ac-btn--agent \.ac-icon/);
    }
  });
});
