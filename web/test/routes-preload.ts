import { existsSync } from "node:fs";
import { Generator, getConfig } from "@tanstack/router-generator";

const root = new URL("..", import.meta.url).pathname;
if (!existsSync(new URL("../src/routeTree.gen.ts", import.meta.url))) {
  const config = getConfig({ target: "react", autoCodeSplitting: true }, root);
  await new Generator({ config, root }).run();
}
