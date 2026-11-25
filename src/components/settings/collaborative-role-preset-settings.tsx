"use client"

import { useEffect, useMemo, useState } from "react"
import { Plus, Trash2, UsersRound } from "lucide-react"
import { useTranslations } from "next-intl"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import {
  buildBuiltInRolePresets,
  makeId,
  readUserRolePresets,
  slugifyRoleKey,
  subscribeUserRolePresets,
  writeUserRolePresets,
  type CollaborativeRolePreset,
} from "@/lib/collaborative-role-presets"
import { AGENT_LABELS, type AgentType } from "@/lib/types"

const AGENT_OPTIONS: AgentType[] = [
  "claude_code",
  "codex",
  "open_code",
  "gemini",
  "open_claw",
  "cline",
  "hermes",
]

export function CollaborativeRolePresetSettingsSection() {
  const t = useTranslations("GeneralSettings.rolePresets")
  const tCreate = useTranslations("CollaborativeProject.createDialog")
  const [userPresets, setUserPresets] = useState<CollaborativeRolePreset[]>([])
  const [name, setName] = useState("")
  const [roleKey, setRoleKey] = useState("")
  const [agentType, setAgentType] = useState<AgentType>("claude_code")
  const [instructions, setInstructions] = useState("")

  const builtInPresets = useMemo(
    () =>
      buildBuiltInRolePresets({
        managerName: tCreate("roles.manager.name"),
        managerInstructions: tCreate("roles.manager.instructions"),
        frontendName: tCreate("roles.frontend.name"),
        frontendInstructions: tCreate("roles.frontend.instructions"),
        backendName: tCreate("roles.backend.name"),
        backendInstructions: tCreate("roles.backend.instructions"),
        qaName: tCreate("roles.qa.name"),
        qaInstructions: tCreate("roles.qa.instructions"),
      }),
    [tCreate]
  )

  useEffect(() => {
    const refresh = () => setUserPresets(readUserRolePresets())
    refresh()
    return subscribeUserRolePresets(refresh)
  }, [])

  function persist(next: CollaborativeRolePreset[]) {
    writeUserRolePresets(next)
    setUserPresets(readUserRolePresets())
  }

  function handleNameChange(nextName: string) {
    const previousGenerated = slugifyRoleKey(name)
    setName(nextName)
    if (!roleKey || roleKey === previousGenerated) {
      setRoleKey(slugifyRoleKey(nextName))
    }
  }

  function resetForm() {
    setName("")
    setRoleKey("")
    setAgentType("claude_code")
    setInstructions("")
  }

  function addPreset() {
    const trimmedName = name.trim()
    const normalizedKey = slugifyRoleKey(roleKey || name)
    if (!trimmedName || !normalizedKey) return
    if (
      [...builtInPresets, ...userPresets].some(
        (preset) => preset.roleKey === normalizedKey
      )
    ) {
      toast.error(t("duplicateRoleKey"))
      return
    }

    persist([
      ...userPresets,
      {
        id: makeId(),
        name: trimmedName,
        roleKey: normalizedKey,
        agentType,
        instructions,
        builtIn: false,
        isManager: false,
      },
    ])
    resetForm()
    toast.success(t("saved"))
  }

  function patchPreset(id: string, patch: Partial<CollaborativeRolePreset>) {
    const next = userPresets.map((preset) =>
      preset.id === id
        ? {
            ...preset,
            ...patch,
            roleKey:
              patch.roleKey !== undefined
                ? slugifyRoleKey(patch.roleKey)
                : preset.roleKey,
          }
        : preset
    )
    persist(next)
  }

  function removePreset(id: string) {
    persist(userPresets.filter((preset) => preset.id !== id))
  }

  const canAdd =
    name.trim().length > 0 && slugifyRoleKey(roleKey || name).length > 0

  return (
    <section className="rounded-xl border bg-card p-4 space-y-4">
      <div className="flex items-center gap-2">
        <UsersRound className="h-4 w-4 text-muted-foreground" />
        <h2 className="text-sm font-semibold">{t("title")}</h2>
      </div>

      <p className="text-xs text-muted-foreground leading-5">
        {t("description")}
      </p>

      <div className="space-y-3">
        <div>
          <div className="text-xs font-medium text-muted-foreground">
            {t("builtInTitle")}
          </div>
          <div className="mt-2 grid gap-2 sm:grid-cols-2">
            {builtInPresets.map((preset) => (
              <div
                key={preset.id}
                className="rounded-lg border bg-muted/20 px-3 py-2"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-sm font-medium">{preset.name}</span>
                  <span className="font-mono text-[11px] text-muted-foreground">
                    {preset.roleKey}
                  </span>
                </div>
                <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                  {preset.instructions}
                </p>
              </div>
            ))}
          </div>
        </div>

        <div className="rounded-lg border bg-muted/10 p-3 space-y-3">
          <div className="text-xs font-medium text-muted-foreground">
            {t("addCustomTitle")}
          </div>
          <div className="grid gap-3 md:grid-cols-[1fr_1fr_180px]">
            <div className="space-y-1.5">
              <Label>{t("roleName")}</Label>
              <Input
                value={name}
                onChange={(event) => handleNameChange(event.target.value)}
                placeholder={t("roleNamePlaceholder")}
              />
            </div>
            <div className="space-y-1.5">
              <Label>{t("roleKey")}</Label>
              <Input
                value={roleKey}
                onChange={(event) =>
                  setRoleKey(slugifyRoleKey(event.target.value))
                }
                placeholder={t("roleKeyPlaceholder")}
              />
            </div>
            <div className="space-y-1.5">
              <Label>{t("agentType")}</Label>
              <Select
                value={agentType}
                onValueChange={(value) => setAgentType(value as AgentType)}
              >
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {AGENT_OPTIONS.map((agent) => (
                    <SelectItem key={agent} value={agent}>
                      {AGENT_LABELS[agent]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <div className="space-y-1.5">
            <Label>{t("instructions")}</Label>
            <Textarea
              value={instructions}
              onChange={(event) => setInstructions(event.target.value)}
              placeholder={t("instructionsPlaceholder")}
              className="min-h-20 resize-y"
            />
          </div>
          <Button
            type="button"
            size="sm"
            onClick={addPreset}
            disabled={!canAdd}
          >
            <Plus className="size-3.5" />
            {t("add")}
          </Button>
        </div>

        {userPresets.length > 0 && (
          <div className="space-y-2">
            <div className="text-xs font-medium text-muted-foreground">
              {t("customTitle")}
            </div>
            {userPresets.map((preset) => (
              <div
                key={preset.id}
                className="rounded-lg border bg-card/60 p-3 space-y-3"
              >
                <div className="grid gap-3 md:grid-cols-[1fr_1fr_180px_auto]">
                  <Input
                    value={preset.name}
                    onChange={(event) =>
                      patchPreset(preset.id, { name: event.target.value })
                    }
                    aria-label={t("roleName")}
                  />
                  <Input
                    value={preset.roleKey}
                    onChange={(event) =>
                      patchPreset(preset.id, {
                        roleKey: event.target.value,
                      })
                    }
                    aria-label={t("roleKey")}
                  />
                  <Select
                    value={preset.agentType}
                    onValueChange={(value) =>
                      patchPreset(preset.id, {
                        agentType: value as AgentType,
                      })
                    }
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {AGENT_OPTIONS.map((agent) => (
                        <SelectItem key={agent} value={agent}>
                          {AGENT_LABELS[agent]}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onClick={() => removePreset(preset.id)}
                    aria-label={t("remove")}
                    title={t("remove")}
                    className="text-muted-foreground hover:text-destructive"
                  >
                    <Trash2 className="size-4" />
                  </Button>
                </div>
                <Textarea
                  value={preset.instructions}
                  onChange={(event) =>
                    patchPreset(preset.id, {
                      instructions: event.target.value,
                    })
                  }
                  aria-label={t("instructions")}
                  className="min-h-16 resize-y"
                />
              </div>
            ))}
          </div>
        )}
      </div>
    </section>
  )
}
