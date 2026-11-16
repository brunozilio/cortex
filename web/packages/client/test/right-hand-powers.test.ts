import { expect, test } from "bun:test";
import { rightHandNoComputer } from "../src/lib/right-hand";

test("why it has no computer for anyone is told by its own line, and any other state by none", () => {
  expect(rightHandNoComputer("no_home")).toBe("rightHand.computerNoHome");
  expect(rightHandNoComputer("not_system")).toBe("rightHand.computerOneUser");
  for (const reason of [null, undefined, "offline", "no_desktop", "not_allowed"]) {
    expect(rightHandNoComputer(reason)).toBeNull();
  }
});
