import { useEffect } from "react"
import { act, render } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"
import {
  AcpConnectionsProvider,
  useAcpActions,
  useConnectionStore,
  type ConnectionStoreApi,
} from "@/contexts/acp-connections-context"
import type { EventEnvelope } from "@/lib/types"

type CapturedAttachHandlers = {
  onSnapshot: (snapshot: unknown, eventSeq: number) => void
  onReplay: (events: EventEnvelope[], highWaterSeq: number) => void
  onEvent: (envelope: EventEnvelope) => void
  onDetached: (reason: "connection_gone" | "lagged" | "server_shutdown") => void
}

// Shared spies + a stub EventStream. `vi.hoisted` runs before the mock
// factories so they can close over this state. Mocking `getEventStream` to a
// non-null stub forces the "web / attach" transport path: the mount listener
// effect sets `listenerReadyRef` synchronously (so `waitForListenerReady` is a
// no-op) and `connectAsViewer` / the owner spawn both route through
// `stream.attach`.
const h = vi.hoisted(() => {
  const attachHandlers = new Map<string, CapturedAttachHandlers>()
  const attach = vi.fn(
    (
      connectionId: string,
      _options: unknown,
      handlers: CapturedAttachHandlers
    ) => {
      attachHandlers.set(connectionId, handlers)
      return {
        subscriptionId: `sub-${connectionId}`,
        detach: vi.fn(() => {
          if (attachHandlers.get(connectionId) === handlers) {
            attachHandlers.delete(connectionId)
          }
        }),
      }
    }
  )
  const stream = { attach }
  const subscribe = vi.fn(
    async (_event: string, handler: (envelope: EventEnvelope) => void) => {
      eventHandler = handler
      return () => {
        if (eventHandler === handler) eventHandler = null
      }
    }
  )
  let eventHandler: ((envelope: EventEnvelope) => void) | null = null
  return {
    attach,
    stream,
    subscribe,
    get eventHandler() {
      return eventHandler
    },
    // getEventStream() returns this — default the web/attach stub; set to null
    // per-test to exercise the desktop firehose path.
    eventStreamValue: stream as { attach: typeof attach } | null,
    actions: null as unknown as ReturnType<typeof useAcpActions> | null,
    store: null as unknown as ConnectionStoreApi | null,
    attachHandlers,
    // api spies
    acpGetAgentStatus: vi.fn(),
    acpFindConnectionForConversation: vi.fn(),
    acpConnect: vi.fn(),
    acpDisconnect: vi.fn(),
    acpGetSessionSnapshot: vi.fn(),
    buildDelegationSeedEnvelopes: vi.fn(() => []),
  }
})

vi.mock("next-intl", () => ({
  useTranslations: () => (key: string) => key,
}))

vi.mock("@/lib/platform", () => ({
  subscribe: h.subscribe,
  getEventStream: () => h.eventStreamValue,
}))

vi.mock("@/lib/delegation-seed", () => ({
  buildDelegationSeedEnvelopes: h.buildDelegationSeedEnvelopes,
}))

vi.mock("@/contexts/alert-context", () => ({
  useAlertContext: () => ({ pushAlert: vi.fn() }),
}))

vi.mock("@/contexts/active-folder-context", () => ({
  useActiveFolder: () => ({ activeFolder: { path: "/tmp/x", name: "x" } }),
}))

vi.mock("@/lib/notification", () => ({
  sendSystemNotification: vi.fn(() => Promise.resolve()),
}))

vi.mock("@/lib/selector-prefs-storage", () => ({
  getSavedPrefsForConnect: () => ({ modeId: undefined, configValues: {} }),
  saveModePreference: vi.fn(),
  saveConfigPreference: vi.fn(),
}))

vi.mock("@/lib/snapshot-denormalize", () => ({
  denormalizeSnapshot: vi.fn(() => ({
    connectionId: "owner-conn",
    eventSeq: 0,
    activeDelegations: [],
  })),
}))

vi.mock("@/lib/api", () => ({
  acpGetAgentStatus: h.acpGetAgentStatus,
  acpFindConnectionForConversation: h.acpFindConnectionForConversation,
  acpConnect: h.acpConnect,
  acpDisconnect: h.acpDisconnect,
  acpGetSessionSnapshot: h.acpGetSessionSnapshot,
  acpPrompt: vi.fn(),
  acpSetMode: vi.fn(),
  acpSetConfigOption: vi.fn(),
  acpCancel: vi.fn(),
  acpRespondPermission: vi.fn(),
  acpTouchConnection: vi.fn(),
}))

function Probe() {
  const actions = useAcpActions()
  const store = useConnectionStore()
  // Capture in an effect (not during render) so the lint rule that forbids
  // mutating external state mid-render stays happy; mountProvider flushes
  // effects before any test reads h.actions.
  useEffect(() => {
    h.actions = actions
    h.store = store
  }, [actions, store])
  return null
}

async function mountProvider() {
  render(
    <AcpConnectionsProvider>
      <Probe />
    </AcpConnectionsProvider>
  )
  await act(async () => {})
}

const TAB = "conv-1-claude_code-42"

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

beforeEach(() => {
  h.attach.mockClear()
  h.attachHandlers.clear()
  h.subscribe.mockClear()
  h.eventStreamValue = h.stream
  h.store = null
  h.buildDelegationSeedEnvelopes.mockClear()
  h.acpGetAgentStatus.mockReset()
  h.acpFindConnectionForConversation.mockReset()
  h.acpConnect.mockReset()
  h.acpDisconnect.mockReset()
  h.acpGetSessionSnapshot.mockReset()
  // Agent is installed + available so the connect preflight passes.
  h.acpGetAgentStatus.mockResolvedValue({
    agent_type: "claude_code",
    enabled: true,
    available: true,
    installed_version: "1.0.0",
  })
  h.acpConnect.mockResolvedValue("spawned-conn")
  h.acpDisconnect.mockResolvedValue(undefined)
  h.acpGetSessionSnapshot.mockResolvedValue(null)
})

describe("AcpConnectionsProvider cross-client viewer lifecycle", () => {
  it("attaches as a viewer (no spawn) when a live connection is discovered", async () => {
    h.acpFindConnectionForConversation.mockResolvedValue({
      connection_id: "owner-conn",
      event_seq: 5,
    })
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1", 42)
    })

    // Discovery ran for the conversation (with the sessionId + agentType
    // fallback), and we attached to the owner's connection instead of spawning.
    expect(h.acpFindConnectionForConversation).toHaveBeenCalledWith(
      42,
      "sess-1",
      "claude_code"
    )
    expect(h.acpConnect).not.toHaveBeenCalled()
    // COLD attach: a viewer has applied no prior events, so it must request a
    // full snapshot (sinceSeq undefined) — NOT the discovered event_seq, which
    // could yield only a post-cursor replay and miss all earlier live state.
    expect(h.attach).toHaveBeenCalledWith(
      "owner-conn",
      { sinceSeq: undefined },
      expect.anything()
    )
  })

  it("spawns + owns when no live connection is discovered", async () => {
    h.acpFindConnectionForConversation.mockResolvedValue(null)
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1", 42)
    })

    expect(h.acpFindConnectionForConversation).toHaveBeenCalledWith(
      42,
      "sess-1",
      "claude_code"
    )
    expect(h.acpConnect).toHaveBeenCalledTimes(1)
    expect(h.attach).toHaveBeenCalledWith(
      "spawned-conn",
      expect.anything(),
      expect.anything()
    )
  })

  it("marks a connecting attach stream as error when the backend emits a terminal error", async () => {
    h.acpFindConnectionForConversation.mockResolvedValue(null)
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1", 42)
    })

    expect(h.store!.getConnection(TAB)?.status).toBe("connecting")

    await act(async () => {
      h.attachHandlers.get("spawned-conn")?.onEvent({
        type: "error",
        connection_id: "spawned-conn",
        seq: 1,
        agent_type: "claude_code",
        code: "initialize_timeout",
        message: "timed out",
      })
    })

    expect(h.store!.getConnection(TAB)?.status).toBe("error")
  })

  it("skips discovery entirely when no persisted conversationId is given", async () => {
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1")
    })

    expect(h.acpFindConnectionForConversation).not.toHaveBeenCalled()
    expect(h.acpConnect).toHaveBeenCalledTimes(1)
  })

  it("viewer teardown detaches WITHOUT acpDisconnect (never kills the owner's agent)", async () => {
    h.acpFindConnectionForConversation.mockResolvedValue({
      connection_id: "owner-conn",
      event_seq: 0,
    })
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1", 42)
    })
    expect(h.acpConnect).not.toHaveBeenCalled()

    await act(async () => {
      await h.actions!.disconnect(TAB)
    })

    // The critical safety property: a viewer must never disconnect the backend
    // connection — it belongs to another client.
    expect(h.acpDisconnect).not.toHaveBeenCalled()
  })

  it("replacing a viewer (changed params) detaches WITHOUT acpDisconnect", async () => {
    // A re-connect at the same tab with a different workingDir hits the
    // replace-existing path. If the existing entry is a viewer, that path must
    // NOT acpDisconnect the owner's connection.
    h.acpFindConnectionForConversation.mockResolvedValue({
      connection_id: "owner-conn",
      event_seq: 0,
    })
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1", 42)
    })
    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/other", "sess-1", 42)
    })

    expect(h.acpDisconnect).not.toHaveBeenCalled()
  })

  it("owner teardown DOES acpDisconnect its own connection", async () => {
    h.acpFindConnectionForConversation.mockResolvedValue(null)
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1", 42)
    })
    expect(h.acpConnect).toHaveBeenCalledTimes(1)

    await act(async () => {
      await h.actions!.disconnect(TAB)
    })

    expect(h.acpDisconnect).toHaveBeenCalledWith("spawned-conn")
  })

  it("replays a same-params connect when the in-flight connect was abandoned", async () => {
    const firstConnect = deferred<string>()
    h.acpConnect
      .mockReturnValueOnce(firstConnect.promise)
      .mockResolvedValueOnce("conn-2")
    await mountProvider()

    let originalConnect: Promise<void> | undefined
    await act(async () => {
      originalConnect = h.actions!.connect(
        TAB,
        "claude_code",
        "/tmp/x",
        "sess-1"
      )
    })

    await act(async () => {
      await h.actions!.disconnect(TAB)
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1")
    })

    await act(async () => {
      firstConnect.resolve("conn-1")
      await originalConnect
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(h.acpDisconnect).toHaveBeenCalledWith("conn-1")
    expect(h.acpConnect).toHaveBeenCalledTimes(2)
    expect(h.store!.getConnection(TAB)?.connectionId).toBe("conn-2")
  })

  it("reconnects when the resolved historical sessionId differs from the live connection", async () => {
    h.acpConnect.mockResolvedValueOnce("conn-1").mockResolvedValueOnce("conn-2")
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x")
    })
    expect(h.store!.getConnection(TAB)?.connectionId).toBe("conn-1")

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "historical")
    })

    expect(h.acpDisconnect).toHaveBeenCalledWith("conn-1")
    expect(h.acpConnect).toHaveBeenCalledTimes(2)
    expect(h.acpConnect).toHaveBeenLastCalledWith(
      "claude_code",
      "/tmp/x",
      "historical",
      undefined,
      {}
    )
    expect(h.store!.getConnection(TAB)?.connectionId).toBe("conn-2")
  })

  it("does not reconnect when a late requested sessionId matches the running session", async () => {
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x")
    })
    await act(async () => {
      h.attachHandlers.get("spawned-conn")?.onEvent({
        type: "session_started",
        connection_id: "spawned-conn",
        seq: 1,
        session_id: "live-session",
      })
    })

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "live-session")
    })

    expect(h.acpDisconnect).not.toHaveBeenCalled()
    expect(h.acpConnect).toHaveBeenCalledTimes(1)
    expect(h.store!.getConnection(TAB)?.status).toBe("connected")
  })

  it("desktop viewer torn down DURING snapshot fetch does not seed delegations or route", async () => {
    // Desktop firehose path (no EventStream). If the viewer's tab disconnects
    // while acpGetSessionSnapshot is in flight, the resumed attach must NOT
    // hydrate / seed child delegation streams / install reverse-map routing for
    // a viewer that no longer exists.
    h.eventStreamValue = null
    h.acpFindConnectionForConversation.mockResolvedValue({
      connection_id: "owner-conn",
      event_seq: 0,
    })
    let resolveSnapshot: (v: unknown) => void = () => {}
    h.acpGetSessionSnapshot.mockImplementation(
      () =>
        new Promise((res) => {
          resolveSnapshot = res
        })
    )
    await mountProvider()

    // Start the viewer connect; it suspends on the pending snapshot AFTER
    // dispatching CONNECTION_CREATED (the entry now exists in the store).
    let connectPromise: Promise<void> | undefined
    await act(async () => {
      connectPromise = h.actions!.connect(TAB, "claude_code", "/tmp/x", "s", 42)
    })
    // Tear the viewer down while the snapshot is still in flight.
    await act(async () => {
      await h.actions!.disconnect(TAB)
    })
    // Snapshot resolves only AFTER teardown; the resumed attach must bail.
    await act(async () => {
      resolveSnapshot({ connection_id: "owner-conn" })
      await connectPromise
    })

    expect(h.buildDelegationSeedEnvelopes).not.toHaveBeenCalled()
    // And teardown never killed the owner's connection.
    expect(h.acpDisconnect).not.toHaveBeenCalled()
  })

  it("desktop firehose fans one delegated role connection out to both the role tab and the delegation card child", async () => {
    // Regression: project role tabs and project-manager delegation cards can
    // point at the same backend child connection. The Tauri firehose used to
    // keep only one reverse-map entry, so attachDelegationChild overwrote the
    // role tab's route and the role tab stopped receiving live status/events.
    h.eventStreamValue = null
    h.acpFindConnectionForConversation.mockResolvedValue(null)
    h.acpConnect.mockResolvedValue("role-child-conn")
    await mountProvider()

    await act(async () => {
      await h.actions!.connect(TAB, "claude_code", "/tmp/x", "sess-1", 42)
    })
    await act(async () => {
      h.actions!.attachDelegationChild({
        connectionId: "role-child-conn",
        parentConnectionId: "pm-conn",
        parentToolUseId: "tool-1",
        agentType: "claude_code",
      })
    })

    expect(h.store!.getConnection(TAB)?.status).toBe("connecting")
    expect(h.store!.getConnection("role-child-conn")?.status).toBe("connected")

    await act(async () => {
      h.eventHandler?.({
        type: "status_changed",
        connection_id: "role-child-conn",
        seq: 1,
        status: "prompting",
      })
    })

    expect(h.store!.getConnection(TAB)?.status).toBe("prompting")
    expect(h.store!.getConnection("role-child-conn")?.status).toBe("prompting")
  })
})
