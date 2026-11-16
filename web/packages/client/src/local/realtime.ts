import {
  RealtimeConnection,
  realtimeUrl,
  type SocketBase,
  type SocketFactory,
  type Timers,
} from "../realtime/connection";
import { pageLocation } from "./page";
import { LOCAL_ORG_ID } from "./stubs";

export interface LocalRealtimeOptions {
  orgId?: string;
  base?: SocketBase;
  createSocket?: SocketFactory;
  timers?: Timers;
  pingMs?: number;
  pongTimeoutMs?: number;
  backoffMs?: number[];
}

const browserSocket: SocketFactory = (url) => new WebSocket(url) as unknown as ReturnType<SocketFactory>;

const pageBase = (): SocketBase => {
  const page = pageLocation();
  if (!page) throw new Error("LocalRealtime needs a base outside a browser");
  return { protocol: page.protocol, host: page.host };
};

export const localRealtimeUrl = (orgId: string = LOCAL_ORG_ID, base: SocketBase = pageBase()) =>
  realtimeUrl(base, orgId);

export class LocalRealtime extends RealtimeConnection {
  readonly orgId: string;
  readonly url: string;

  constructor(options: LocalRealtimeOptions = {}) {
    const orgId = options.orgId ?? LOCAL_ORG_ID;
    const url = localRealtimeUrl(orgId, options.base ?? pageBase());
    super({
      url,
      createSocket: options.createSocket ?? browserSocket,
      ...(options.timers ? { timers: options.timers } : {}),
      ...(options.pingMs !== undefined ? { pingMs: options.pingMs } : {}),
      ...(options.pongTimeoutMs !== undefined ? { pongTimeoutMs: options.pongTimeoutMs } : {}),
      ...(options.backoffMs ? { backoffMs: options.backoffMs } : {}),
    });
    this.orgId = orgId;
    this.url = url;
  }
}

export const createLocalRealtime = (options: LocalRealtimeOptions = {}) => new LocalRealtime(options);
