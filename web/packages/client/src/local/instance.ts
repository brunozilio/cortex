import type { FetchLike } from "../api/http";
import { ApiError } from "../api/http";
import type { Health, Instance } from "../api/types";
import { APP_API } from "../lib/server";
import { pageLocation } from "./page";

export const INSTANCE_PATH = "/api/instance";
export const HEALTH_PATH = "/api/health";

export interface LocalTransport {
  fetch: FetchLike;
  baseUrl: string;
}

export function localTransport(options: { fetch?: FetchLike; baseUrl?: string } = {}): LocalTransport {
  return {
    fetch: options.fetch ?? ((input, init) => globalThis.fetch(input, init)),
    baseUrl: (options.baseUrl ?? "").replace(/\/+$/, ""),
  };
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const text = (value: unknown, fallback: string) => (typeof value === "string" ? value : fallback);
const flag = (value: unknown, fallback: boolean) => (typeof value === "boolean" ? value : fallback);
const count = (value: unknown, fallback: number) =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;

const here = () => pageLocation()?.origin ?? "";

export function instanceOf(body: unknown, origin: string = here()): Instance {
  const raw = isRecord(body) ? body : {};
  const signIn = isRecord(raw.signIn) ? raw.signIn : {};
  const instance: Instance = {
    name: text(raw.name, ""),
    version: text(raw.version, ""),
    api: count(raw.api, APP_API),
    minApi: count(raw.minApi, APP_API),
    url: text(raw.url, origin),
    signIn: {
      email: flag(signIn.email, false),
      google: flag(signIn.google, false),
      github: flag(signIn.github, false),
      apple: flag(signIn.apple, false),
    },
    signup: raw.signup === "open" ? "open" : "invite",
    ownerClaim: flag(raw.ownerClaim, false),
    cloud: flag(raw.cloud, false),
  };
  if (typeof raw.sourceUrl === "string") instance.sourceUrl = raw.sourceUrl;
  return instance;
}

async function getJson(transport: LocalTransport, path: string): Promise<{ status: number; body: unknown }> {
  const res = await transport.fetch(`${transport.baseUrl}${path}`, {
    method: "GET",
    headers: { accept: "application/json" },
    credentials: "same-origin",
  });
  const body: unknown = await res.json().catch(() => null);
  return { status: res.status, body };
}

const errorCode = (body: unknown, fallback: string) => {
  const error = isRecord(body) && isRecord(body.error) ? body.error : {};
  return text(error.code, fallback);
};

export async function fetchInstance(transport: LocalTransport): Promise<Instance> {
  const { status, body } = await getJson(transport, INSTANCE_PATH);
  if (status < 200 || status >= 300) throw new ApiError(status, errorCode(body, "instance_unavailable"), "Instance");
  return instanceOf(body);
}

export async function fetchHealth(transport: LocalTransport): Promise<Health> {
  try {
    const { status, body } = await getJson(transport, HEALTH_PATH);
    const raw = isRecord(body) ? body : {};
    return {
      status: status >= 200 && status < 300 && raw.status !== "degraded" ? "ok" : "degraded",
      version: text(raw.version, ""),
      edition: "selfhost",
    };
  } catch {
    return { status: "degraded", version: "", edition: "selfhost" };
  }
}
