import { expect, test } from "bun:test";
import { resources } from "@agenteschat/i18n";
import i18next from "i18next";
import { ApiError } from "../src/index";
import { overLimitText, tooLongRefusal, tooLongToSend } from "../src/lib/message-length";


test("a field's limit counts as the server does, and its line says the size, the limit and the way out", () => {
  expect(tooLongToSend(` ${"x".repeat(300)} `, 300)).toBe(false);
  expect(tooLongToSend("x".repeat(301), 300)).toBe(true);
  const i18n = i18next.createInstance();
  void i18n.init({ lng: "pt-BR", resources, initAsync: false, interpolation: { escapeValue: false } });
  expect(overLimitText(i18n.t, "pt-BR", 20_512, 20_000)).toBe(
    "O texto tem 20.512 caracteres e o limite é 20.000. Encurte o texto.",
  );
  const refused = new ApiError(400, "message.too_long", "Message is longer than 100000 characters", {
    length: 100_412,
    max: 100_000,
  });
  expect(tooLongRefusal(i18n.t, "pt-BR", refused, "Encurte a resposta.")).toBe(
    "A mensagem tem 100.412 caracteres e o limite é 100.000. Encurte a resposta.",
  );
  expect(tooLongRefusal(i18n.t, "pt-BR", new ApiError(500, "internal", "boom"), "Encurte a resposta.")).toBeNull();
});
