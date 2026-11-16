import { afterEach, describe, expect, test } from "bun:test";
import { setupDom } from "@agenteschat/ui/testing/dom";

setupDom();

import type { AgentGroup } from "@agenteschat/client";
import { act, cleanup, fireEvent, within } from "@testing-library/react";
import { ADA_ID, ATLAS_ID, createFakeApi, GENERAL_ID, ORG_ID, seedState } from "./fake-api";
import { renderApp, screen, settle } from "./render";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const click = async (el: Element) => {
  await act(async () => fireEvent.click(el));
  await settle(3);
};
const INTEGRATIONS = "00000000-0000-4000-8000-0000000000e1";

/** Acme with a group, Integrations, holding Atlas (Ada in none); `role`: mine. */
function withGroups(groups: AgentGroup[] = [{ id: INTEGRATIONS, name: "Integrations", agentIds: [ATLAS_ID] }]) {
  const state = seedState();
  state.agentGroups = groups;
  return state;
}
const asMember = (state = seedState()) => {
  if (state.me?.organizations[0]) state.me.organizations[0].role = "member";
  return state;
};
const go = async (state = seedState(), path = `c/${GENERAL_ID}`) => {
  const r = await renderApp(`/org/${ORG_ID}/${path}`, { api: createFakeApi(state) });
  await settle(8);
  return { ...r, state };
};
describe("the Group field on the agent's page", () => {
  test("owners and admins pick it in Configurações and it saves with the page", async () => {
    const { state } = await go(withGroups(), `agents/${ADA_ID}?tab=settings&sec=identity`);
    const field = screen().getByRole("combobox", { name: "Setor" });
    expect(field.textContent).toBe("Sem setor");
    field.focus();
    await act(async () => fireEvent.keyDown(field, { key: "Enter" }));
    await click(screen().getByRole("option", { name: "Integrations" }));
    const bar = screen().getByRole("region", { name: "Alterações não salvas" });
    expect(bar.textContent).toContain("Setor");
    await click(within(bar).getByRole("button", { name: /^Salvar/ }));
    await settle(4);
    expect(calls(state, "agentGroups.move")).toEqual([[ADA_ID, { groupId: INTEGRATIONS }]]);
    expect(screen().getByRole("combobox", { name: "Setor" }).textContent).toBe("Integrations");
  });

  test("with no groups there is no field; a member reads the agent's group", async () => {
    await go(seedState(), `agents/${ADA_ID}?tab=settings&sec=identity`);
    expect(screen().queryByRole("combobox", { name: "Setor" })).toBeNull();
    cleanup();
    await go(asMember(withGroups()), `agents/${ATLAS_ID}?tab=settings&sec=identity`);
    expect(screen().queryByRole("combobox", { name: "Setor" })).toBeNull();
    expect(document.querySelector(".agp-id")?.textContent).toContain("Integrations");
  });
});
