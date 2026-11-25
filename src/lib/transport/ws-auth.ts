export const BILLOW_WS_PROTOCOL = "billow-events"
const BILLOW_WS_TOKEN_PROTOCOL_PREFIX = "billow-token."

function base64UrlEncode(value: string): string {
  const bytes = new TextEncoder().encode(value)
  let binary = ""
  for (const byte of bytes) {
    binary += String.fromCharCode(byte)
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "")
}

export function buildBillowWebSocketProtocols(token: string): string[] {
  const trimmed = token.trim()
  if (!trimmed) return [BILLOW_WS_PROTOCOL]
  return [
    BILLOW_WS_PROTOCOL,
    `${BILLOW_WS_TOKEN_PROTOCOL_PREFIX}${base64UrlEncode(trimmed)}`,
  ]
}
