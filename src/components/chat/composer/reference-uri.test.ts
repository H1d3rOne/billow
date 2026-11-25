import { describe, expect, it } from "vitest"

import {
  buildEmbeddedReferenceUri,
  isEmbeddedReferenceUri,
  parseBillowReferenceUri,
} from "./reference-uri"

describe("parseBillowReferenceUri", () => {
  it("returns null for non-reference schemes", () => {
    expect(parseBillowReferenceUri("https://example.com", "x")).toBeNull()
    expect(parseBillowReferenceUri("data:text/plain,abc", "x")).toBeNull()
    expect(parseBillowReferenceUri("billow://unknown/1", "x")).toBeNull()
  })

  it("parses a file uri, falling back to the basename when label is empty", () => {
    expect(
      parseBillowReferenceUri("file:///repo/deep/name.ts", "")
    ).toMatchObject({
      refType: "file",
      id: "name.ts",
      label: "name.ts",
      uri: "file:///repo/deep/name.ts",
      meta: { fileKind: "file" },
    })
  })

  it("parses an agent uri, stripping a leading @ from the label", () => {
    expect(
      parseBillowReferenceUri("billow://agent/codex", "@Codex")
    ).toMatchObject({
      refType: "agent",
      id: "codex",
      label: "Codex",
      uri: "billow://agent/codex",
      meta: { agentType: "codex" },
    })
  })

  it("falls back to the agent type when the agent label is empty", () => {
    expect(
      parseBillowReferenceUri("billow://agent/claude_code", "")
    ).toMatchObject({
      refType: "agent",
      id: "claude_code",
      label: "claude_code",
      meta: { agentType: "claude_code" },
    })
  })

  it("parses a new-format session uri, recovering the agent type", () => {
    expect(
      parseBillowReferenceUri("billow://session/codex_abc123", "My chat")
    ).toMatchObject({
      refType: "session",
      id: "codex_abc123",
      label: "My chat",
      uri: "billow://session/codex_abc123",
      meta: { agentType: "codex" },
    })
  })

  it("never splits an agent type on its first underscore", () => {
    // claude_code / open_code / open_claw contain underscores; a naive first-`_`
    // split would yield "claude" / "open". The whole `<type>_<external_id>` is
    // the id and the full type is recovered by prefix match.
    expect(
      parseBillowReferenceUri("billow://session/claude_code_sess-9", "")
    ).toMatchObject({
      id: "claude_code_sess-9",
      meta: { agentType: "claude_code" },
    })
    expect(
      parseBillowReferenceUri("billow://session/open_code_x", "")?.meta
    ).toEqual({ agentType: "open_code" })
    expect(
      parseBillowReferenceUri("billow://session/open_claw_y", "")?.meta
    ).toEqual({ agentType: "open_claw" })
  })

  it("treats a legacy numeric session id as opaque (no agent icon)", () => {
    expect(
      parseBillowReferenceUri("billow://session/123", "Login")
    ).toMatchObject({
      refType: "session",
      id: "123",
      label: "Login",
      uri: "billow://session/123",
      meta: null,
    })
  })

  it("treats a non-agent-prefixed token as a plain session id", () => {
    expect(
      parseBillowReferenceUri("billow://session/randomtoken", "")
    ).toMatchObject({ refType: "session", id: "randomtoken", meta: null })
  })

  it("falls back to #id for an empty session label", () => {
    expect(parseBillowReferenceUri("billow://session/123", "")?.label).toBe(
      "#123"
    )
  })

  it("parses a commit uri, deriving the short hash", () => {
    expect(
      parseBillowReferenceUri("billow://commit/%2Frepo@abc1234def5678", "abc1234")
    ).toMatchObject({
      refType: "commit",
      id: "abc1234def5678",
      label: "abc1234",
      uri: "billow://commit/%2Frepo@abc1234def5678",
      meta: { shortHash: "abc1234" },
    })
  })

  it("parses a skill uri, keeping the literal `/`·`$` token as the label", () => {
    expect(
      parseBillowReferenceUri("billow://skill/review", "/review")
    ).toMatchObject({
      refType: "skill",
      id: "review",
      label: "/review",
      uri: "billow://skill/review",
      meta: null,
    })
  })

  it("falls back to a /-prefixed id for an empty skill label", () => {
    expect(parseBillowReferenceUri("billow://skill/deploy", "")?.label).toBe(
      "/deploy"
    )
  })

  it("parses an embedded-attachment uri as an inert file badge", () => {
    expect(
      parseBillowReferenceUri("billow://embedded/9f3c-uuid", "report.pdf")
    ).toMatchObject({
      refType: "file",
      label: "report.pdf",
      uri: "billow://embedded/9f3c-uuid",
      meta: { fileKind: "file" },
    })
  })

  it("falls back to a generic label for an empty embedded-attachment label", () => {
    expect(
      parseBillowReferenceUri("billow://embedded/9f3c-uuid", "")?.label
    ).toBe("resource")
  })

  it("recognizes a freshly minted embedded reference uri", () => {
    const uri = buildEmbeddedReferenceUri()
    expect(isEmbeddedReferenceUri(uri)).toBe(true)
    expect(isEmbeddedReferenceUri("file:///billow-embedded/real.ts")).toBe(false)
    expect(isEmbeddedReferenceUri("billow://session/abc")).toBe(false)
  })
})
