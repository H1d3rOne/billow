import type { AgentType } from "@/lib/types"

export interface CollaborativeRolePreset {
  id: string
  name: string
  roleKey: string
  agentType: AgentType
  instructions: string
  builtIn?: boolean
  isManager?: boolean
}

export interface BuiltInRolePresetLabels {
  managerName: string
  managerInstructions: string
  frontendName: string
  frontendInstructions: string
  backendName: string
  backendInstructions: string
  qaName: string
  qaInstructions: string
}

export const COLLABORATIVE_ROLE_PRESETS_STORAGE_KEY =
  "billow.collaborative_role_presets.v1"

export const ROLE_PRESETS_CHANGED_EVENT =
  "billow:collaborative-role-presets-changed"

const AGENT_TYPES = new Set<AgentType>([
  "claude_code",
  "codex",
  "open_code",
  "gemini",
  "open_claw",
  "cline",
  "hermes",
])

export function slugifyRoleKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9_-]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 64)
}

export function makeId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID()
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`
}

export function makeUniqueRoleKey(
  preferred: string,
  existingRoleKeys: Set<string>
): string {
  const base = slugifyRoleKey(preferred) || "custom_role"
  if (!existingRoleKeys.has(base)) return base

  let index = 2
  let next = `${base}_${index}`
  while (existingRoleKeys.has(next)) {
    index += 1
    next = `${base}_${index}`
  }
  return next
}

export function buildBuiltInRolePresets(
  labels: BuiltInRolePresetLabels
): CollaborativeRolePreset[] {
  return [
    {
      id: "builtin:project_manager",
      name: labels.managerName,
      roleKey: "project_manager",
      agentType: "claude_code",
      instructions: labels.managerInstructions,
      builtIn: true,
      isManager: true,
    },
    {
      id: "builtin:frontend",
      name: labels.frontendName,
      roleKey: "frontend",
      agentType: "claude_code",
      instructions: labels.frontendInstructions,
      builtIn: true,
      isManager: false,
    },
    {
      id: "builtin:backend",
      name: labels.backendName,
      roleKey: "backend",
      agentType: "claude_code",
      instructions: labels.backendInstructions,
      builtIn: true,
      isManager: false,
    },
    {
      id: "builtin:qa",
      name: labels.qaName,
      roleKey: "qa",
      agentType: "claude_code",
      instructions: labels.qaInstructions,
      builtIn: true,
      isManager: false,
    },
  ]
}

function normalizeUserPreset(value: unknown): CollaborativeRolePreset | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null
  const record = value as Record<string, unknown>
  const name = typeof record.name === "string" ? record.name.trim() : ""
  const roleKey = slugifyRoleKey(
    typeof record.roleKey === "string" ? record.roleKey : ""
  )
  const agentType = record.agentType
  if (!name || !roleKey || !AGENT_TYPES.has(agentType as AgentType)) {
    return null
  }

  return {
    id: typeof record.id === "string" && record.id ? record.id : makeId(),
    name,
    roleKey,
    agentType: agentType as AgentType,
    instructions:
      typeof record.instructions === "string" ? record.instructions : "",
    builtIn: false,
    isManager: false,
  }
}

export function readUserRolePresets(): CollaborativeRolePreset[] {
  if (typeof window === "undefined") return []
  try {
    const raw = window.localStorage.getItem(
      COLLABORATIVE_ROLE_PRESETS_STORAGE_KEY
    )
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed
      .map((item) => normalizeUserPreset(item))
      .filter((item): item is CollaborativeRolePreset => item !== null)
  } catch {
    return []
  }
}

export function writeUserRolePresets(presets: CollaborativeRolePreset[]): void {
  if (typeof window === "undefined") return
  const normalized = presets
    .map((preset) => normalizeUserPreset(preset))
    .filter((item): item is CollaborativeRolePreset => item !== null)
  window.localStorage.setItem(
    COLLABORATIVE_ROLE_PRESETS_STORAGE_KEY,
    JSON.stringify(normalized)
  )
  window.dispatchEvent(new Event(ROLE_PRESETS_CHANGED_EVENT))
}

export function subscribeUserRolePresets(listener: () => void): () => void {
  if (typeof window === "undefined") return () => {}
  const onStorage = (event: StorageEvent) => {
    if (event.key === COLLABORATIVE_ROLE_PRESETS_STORAGE_KEY) listener()
  }
  window.addEventListener("storage", onStorage)
  window.addEventListener(ROLE_PRESETS_CHANGED_EVENT, listener)
  return () => {
    window.removeEventListener("storage", onStorage)
    window.removeEventListener(ROLE_PRESETS_CHANGED_EVENT, listener)
  }
}
