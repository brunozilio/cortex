import { afterEach, describe, expect, test } from "bun:test";
import { setupDom } from "@agenteschat/ui/testing/dom";

setupDom();

import { cleanup, within } from "@testing-library/react";
import { NewChannelDialog } from "@/components/NewChannelDialog";
import { createFakeApi, seedState } from "./fake-api";
import { renderPage, screen, settle } from "./render";

afterEach(() => cleanup());

describe("a group conversation in a personal workspace (ADR 0159)", () => {
  /** The seeded workspace as a personal one; `alone`: Walter its only member. */
  const personal = (alone: boolean) => {
    const state = seedState();
    const org = state.me?.organizations[0];
    if (org) org.mode = "personal";
    if (alone) state.members = state.members.filter((m) => m.userId === state.me?.user.id);
    return state;
  };
  const openDialog = async (alone: boolean) => {
    await renderPage(<NewChannelDialog open onOpenChange={() => {}} />, {
      api: createFakeApi(personal(alone)),
      org: true,
    });
    await settle(4);
    return screen().getByRole("dialog", { name: "Nova conversa em grupo" });
  };

  test("alone, who can join stays open to everyone here, out of sight; the same channel is created", async () => {
    const dialog = await openDialog(true);
    expect(within(dialog).queryByRole("combobox", { name: "Quem pode entrar" })).toBeNull();
    expect(dialog.textContent).toContain("Quem participa");
  });

});
