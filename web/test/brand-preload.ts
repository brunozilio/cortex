import i18next, { type InitOptions } from "i18next";

type Init = (this: unknown, options?: InitOptions | (() => void), callback?: () => void) => unknown;

const TEST_BRAND = "agentes.chat";
const proto = Object.getPrototypeOf(i18next) as { init: Init };
const init = proto.init;

proto.init = function (this: unknown, options, callback) {
  if (typeof options === "function") return init.call(this, options);
  const given = options ?? {};
  return init.call(
    this,
    {
      ...given,
      interpolation: {
        ...given.interpolation,
        defaultVariables: { brand: TEST_BRAND, ...given.interpolation?.defaultVariables },
      },
    },
    callback,
  );
};
