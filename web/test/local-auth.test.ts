import { describe, expect, test } from "bun:test";
import { createLocalAuthClient, LOCAL_AUTH_REFUSAL } from "@/auth/local";

describe("the local sign-in", () => {
  test("every sign-in step is refused as not available, without a request", async () => {
    const calls: unknown[] = [];
    const real = globalThis.fetch;
    globalThis.fetch = (async (...args: unknown[]) => {
      calls.push(args);
      return new Response(null, { status: 500 });
    }) as unknown as typeof fetch;
    try {
      const auth = createLocalAuthClient();
      expect(await auth.signIn({ email: "a@example.com", password: "senha-longa" })).toEqual(LOCAL_AUTH_REFUSAL);
      expect(await auth.signUp({ name: "A", email: "a@example.com", password: "senha-longa" })).toEqual(
        LOCAL_AUTH_REFUSAL,
      );
      expect(await auth.updateName("A")).toEqual(LOCAL_AUTH_REFUSAL);
      expect(await auth.sendVerificationEmail("a@example.com")).toEqual(LOCAL_AUTH_REFUSAL);
      expect(await auth.signInSocial("github", "/")).toEqual(LOCAL_AUTH_REFUSAL);
      expect(await auth.requestPasswordReset({ email: "a@example.com", redirectTo: "/" })).toEqual(LOCAL_AUTH_REFUSAL);
      expect(await auth.resetPassword({ token: "t", newPassword: "senha-longa" })).toEqual(LOCAL_AUTH_REFUSAL);
      expect(await auth.signInOneTime("t")).toEqual(LOCAL_AUTH_REFUSAL);
      expect(
        await auth.ownerClaim({ token: "t", name: "A", email: "a@example.com", password: "senha-longa" }),
      ).toEqual(LOCAL_AUTH_REFUSAL);
      await auth.signOut();
      expect(calls).toEqual([]);
    } finally {
      globalThis.fetch = real;
    }
  });

  test("its refusal is a 501 the pages can word", () => {
    expect(LOCAL_AUTH_REFUSAL).toEqual({ ok: false, status: 501, code: "not_available", message: "not_available" });
  });
});
