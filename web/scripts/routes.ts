import { Generator, getConfig } from "@tanstack/router-generator";

const root = new URL("..", import.meta.url).pathname;
const config = getConfig({ target: "react", autoCodeSplitting: true }, root);
await new Generator({ config, root }).run();
