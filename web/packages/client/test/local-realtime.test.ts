import { describe, expect, test } from "bun:test";
import type { ConnectionStatus, Timers } from "../src/index";
import { LocalRealtime, localRealtimeUrl } from "../src/local";
import { fakeSockets } from "./fake-socket";

function manualTimers() {
  const pending: { fn: () => void; ms: number; id: number }[] = [];
  const intervals: { fn: () => void; ms: number; id: number }[] = [];
  let next = 1;
  const timers: Timers = {
    setTimeout: (fn, ms) => {
      const id = next++;
      pending.push({ fn, ms, id });
      return id;
    },
    clearTimeout: (id) => {
      const i = pending.findIndex((p) => p.id === id);
      if (i >= 0) pending.splice(i, 1);
    },
    setInterval: (fn, ms) => {
      const id = next++;
      intervals.push({ fn, ms, id });
      return id;
    },
    clearInterval: (id) => {
      const i = intervals.findIndex((p) => p.id === id);
      if (i >= 0) intervals.splice(i, 1);
    },
  };
  return {
    timers,
    pending,
    intervals,
    runNext: () => pending.shift()?.fn(),
    tick: () => {
      for (const i of [...intervals]) i.fn();
    },
  };
}

const page = { protocol: "http:", host: "127.0.0.1:7000" };

function connect() {
  const sockets = fakeSockets();
  const clock = manualTimers();
  const conn = new LocalRealtime({
    base: page,
    createSocket: sockets.factory,
    timers: clock.timers,
  });
  const statuses: ConnectionStatus[] = [];
  conn.onStatus((s) => statuses.push(s));
  return { conn, sockets, clock, statuses };
}

describe("LocalRealtime", () => {
  test("opens /api/ws on the page's own host, ws or wss by protocol", () => {
    expect(localRealtimeUrl("local", page)).toBe("ws://127.0.0.1:7000/api/ws?orgId=local");
    expect(localRealtimeUrl("local", { protocol: "https:", host: "box.lan" })).toBe("wss://box.lan/api/ws?orgId=local");
    const { conn, sockets } = connect();
    conn.start();
    expect(sockets.last().url).toBe("ws://127.0.0.1:7000/api/ws?orgId=local");
    expect(conn.orgId).toBe("local");
  });

  test("subscribes channels once open and hears their events after ready", () => {
    const { conn, sockets, statuses } = connect();
    const heard: unknown[] = [];
    conn.subscribe("c1", (e) => heard.push(e));
    conn.start();
    const socket = sockets.last();
    socket.open();
    expect(statuses.at(-1)).toBe("open");
    expect(socket.sent).toContainEqual({ type: "subscribe", channelId: "c1" });
    socket.push({ type: "ready", orgId: "local", userId: "local" });
    socket.push({ type: "subscribed", channelId: "c1" });
    socket.push({
      type: "event",
      channelId: "c1",
      event: { type: "typing", userId: "u" },
    });
    expect(heard).toHaveLength(1);
  });

  test("unsubscribes when the last listener leaves", () => {
    const { conn, sockets } = connect();
    conn.start();
    sockets.last().open();
    const off = conn.subscribe("c1", () => {});
    off();
    expect(sockets.last().sent).toContainEqual({
      type: "unsubscribe",
      channelId: "c1",
    });
  });

  test("pings on the interval and a pong keeps the socket", () => {
    const { conn, sockets, clock } = connect();
    conn.start();
    const socket = sockets.last();
    socket.open();
    clock.tick();
    expect(socket.sent).toContainEqual({ type: "ping" });
    socket.push({ type: "pong" });
    expect(clock.pending).toHaveLength(0);
    expect(socket.closed).toBe(false);
  });

  test("no pong replaces the socket", () => {
    const { conn, sockets, clock } = connect();
    conn.start();
    sockets.last().open();
    clock.tick();
    clock.runNext();
    expect(sockets.sockets[0]?.closed).toBe(true);
    clock.runNext();
    expect(sockets.sockets).toHaveLength(2);
  });

  test("a socket that can't connect retries with backoff and says reconnecting after the first open", () => {
    const { conn, sockets, clock, statuses } = connect();
    conn.start();
    sockets.last().drop();
    expect(clock.pending[0]?.ms).toBe(500);
    clock.runNext();
    expect(sockets.sockets).toHaveLength(2);
    sockets.last().open();
    sockets.last().drop();
    clock.runNext();
    expect(statuses).toContain("reconnecting");
  });

  test("reconnecting tells listeners to catch up and subscribes again", () => {
    const { conn, sockets, clock } = connect();
    let caughtUp = 0;
    conn.onReconnect(() => caughtUp++);
    conn.subscribe("c1", () => {});
    conn.start();
    sockets.last().open();
    sockets.last().drop();
    clock.runNext();
    const second = sockets.last();
    second.open();
    expect(caughtUp).toBe(1);
    expect(second.sent).toContainEqual({ type: "subscribe", channelId: "c1" });
  });
});
