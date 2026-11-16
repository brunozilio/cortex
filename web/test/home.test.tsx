import { expect, test } from "bun:test";
import { render } from "@testing-library/react";
import { Home } from "../src/routes/index";

test("the home page renders an empty main area", () => {
  const { container } = render(<Home />);
  expect(container.querySelector("main")).not.toBeNull();
});
