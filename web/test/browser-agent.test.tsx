import { afterEach, describe, expect, test } from "bun:test";
import { setupDom } from "@agenteschat/ui/testing/dom";

setupDom();

import { isBlankPage, openRefusalKey } from "@agenteschat/client/lib/sites";
import { cleanup, } from "@testing-library/react";
import { agentSearch } from "@/lib/agent-edit";

afterEach(() => {
  cleanup();
});

describe("browser helpers", () => {
  test("the links, blank pages and what a refused open says", () => {
    // `?computer=1`, and the older links to it.
    for (const name of ["computer", "browser", "desktop", "terminal"]) {
      expect(agentSearch({ [name]: 1 })).toEqual({ tab: undefined, sec: undefined, computer: 1 });
      expect(agentSearch({ [name]: "1", tab: "memory" })).toMatchObject({ tab: "memory", computer: 1 });
    }
    expect(agentSearch({ browser: "yes" })).not.toHaveProperty("computer");
    expect(isBlankPage("about:blank")).toBe(true);
    expect(isBlankPage("https://loja.example.com/")).toBe(false);
    expect(isBlankPage(undefined)).toBe(false);
    expect(openRefusalKey("browser.not_allowed")).toBe("browser.inCard.goNotAllowed");
    expect(openRefusalKey("browser.bad_address")).toBe("browser.inCard.goBadAddress");
    expect(openRefusalKey("machine.timeout")).toBe("browser.live.actionFailed");
    // The workspace's own computer is never off.
    expect(openRefusalKey("machine.offline")).toBe("browser.inCard.offline");
    expect(openRefusalKey("machine.offline", true)).toBe("browser.inCard.offlineCloud");
  });
});
