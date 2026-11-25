"use client"

import { useEffect, useMemo, useState } from "react"
import { useTranslations } from "next-intl"
import { toast } from "sonner"
import { Loader2 } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
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
import { useAppWorkspace } from "@/contexts/app-workspace-context"
import { useOpenConversationTab } from "@/hooks/use-open-conversation-tab"
import { addCollaborativeProjectRole } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import {
  buildBuiltInRolePresets,
  makeUniqueRoleKey,
  readUserRolePresets,
  slugifyRoleKey,
  subscribeUserRolePresets,
  type CollaborativeRolePreset,
} from "@/lib/collaborative-role-presets"
import {
  AGENT_LABELS,
  type AgentType,
  type CollaborativeProjectSummary,
} from "@/lib/types"

const CUSTOM_PRESET_ID = "__custom__"

const AGENT_OPTIONS: AgentType[] = [
  "claude_code",
  "codex",
  "open_code",
  "gemini",
  "open_claw",
  "cline",
  "hermes",
]

interface AddCollaborativeProjectRoleDialogProps {
  project: CollaborativeProjectSummary | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AddCollaborativeProjectRoleDialog({
  project,
  open,
  onOpenChange,
}: AddCollaborativeProjectRoleDialogProps) {
  const t = useTranslations("CollaborativeProject.addRoleDialog")
  const tCreate = useTranslations("CollaborativeProject.createDialog")
  const { refreshConversations, refreshProjects } = useAppWorkspace()
  const openConversationTab = useOpenConversationTab()

  const existingRoleKeys = useMemo(
    () => new Set(project?.roles.map((role) => role.roleKey) ?? []),
    [project]
  )

  const [name, setName] = useState("")
  const [roleKey, setRoleKey] = useState("")
  const [agentType, setAgentType] = useState<AgentType>("claude_code")
  const [instructions, setInstructions] = useState("")
  const [presetId, setPresetId] = useState(CUSTOM_PRESET_ID)
  const [userPresets, setUserPresets] = useState<CollaborativeRolePreset[]>([])
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

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
  const rolePresets = useMemo(
    () =>
      [...builtInPresets, ...userPresets].filter((preset) => !preset.isManager),
    [builtInPresets, userPresets]
  )

  useEffect(() => {
    const refresh = () => setUserPresets(readUserRolePresets())
    refresh()
    return subscribeUserRolePresets(refresh)
  }, [])

  function resetForm() {
    setName("")
    setRoleKey("")
    setAgentType("claude_code")
    setInstructions("")
    setPresetId(CUSTOM_PRESET_ID)
    setError(null)
  }

  useEffect(() => {
    if (!open) resetForm()
  }, [open])

  function handleNameChange(nextName: string) {
    const previousGenerated = slugifyRoleKey(name)
    setName(nextName)
    setPresetId(CUSTOM_PRESET_ID)
    if (!roleKey || roleKey === previousGenerated) {
      setRoleKey(slugifyRoleKey(nextName))
    }
  }

  function applyPreset(nextPresetId: string) {
    setPresetId(nextPresetId)
    if (nextPresetId === CUSTOM_PRESET_ID) return
    const preset = rolePresets.find((item) => item.id === nextPresetId)
    if (!preset) return
    setName(preset.name)
    setRoleKey(makeUniqueRoleKey(preset.roleKey, existingRoleKeys))
    setAgentType(preset.agentType)
    setInstructions(preset.instructions)
  }

  const trimmedName = name.trim()
  const trimmedRoleKey = roleKey.trim()
  const invalidRoleKey =
    trimmedRoleKey.length > 0 && !/^[a-z0-9_-]+$/.test(trimmedRoleKey)
  const duplicateRoleKey = existingRoleKeys.has(trimmedRoleKey)
  const canCreate =
    project !== null &&
    trimmedName.length > 0 &&
    trimmedRoleKey.length > 0 &&
    !invalidRoleKey &&
    !duplicateRoleKey

  async function handleCreate() {
    if (!project || !canCreate || creating) return
    setCreating(true)
    setError(null)
    try {
      const result = await addCollaborativeProjectRole({
        projectId: project.projectId,
        name: trimmedName,
        roleKey: trimmedRoleKey,
        agentType,
        instructions: instructions.trim() || null,
      })
      await refreshConversations()
      await refreshProjects()
      await openConversationTab(result.conversationId, {
        placement: "afterActive",
        title: result.conversationTitle,
      })
      toast.success(t("toasts.created"), {
        description: result.conversationTitle,
      })
      onOpenChange(false)
      resetForm()
    } catch (err) {
      const message = toErrorMessage(err)
      setError(message)
      toast.error(t("toasts.createFailed"), { description: message })
    } finally {
      setCreating(false)
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        onOpenChange(value)
        if (!value) resetForm()
      }}
    >
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("title")}</DialogTitle>
          <DialogDescription>
            {t("description", { name: project?.name ?? "" })}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-1">
          <div className="space-y-1.5">
            <Label>{t("rolePreset")}</Label>
            <Select
              value={presetId}
              onValueChange={applyPreset}
              disabled={creating}
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={CUSTOM_PRESET_ID}>
                  {t("customRole")}
                </SelectItem>
                {rolePresets.map((preset) => (
                  <SelectItem key={preset.id} value={preset.id}>
                    {preset.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              {t("rolePresetHint")}
            </p>
          </div>

          <div className="grid gap-3 sm:grid-cols-[1fr_1fr_180px]">
            <div className="space-y-1.5">
              <Label htmlFor="add-project-role-name">{t("roleName")}</Label>
              <Input
                id="add-project-role-name"
                value={name}
                onChange={(event) => handleNameChange(event.target.value)}
                placeholder={t("roleNamePlaceholder")}
                disabled={creating}
                autoFocus
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="add-project-role-key">{t("roleKey")}</Label>
              <Input
                id="add-project-role-key"
                value={roleKey}
                onChange={(event) => {
                  setRoleKey(event.target.value.trim().toLowerCase())
                  setPresetId(CUSTOM_PRESET_ID)
                }}
                disabled={creating}
                aria-invalid={duplicateRoleKey || invalidRoleKey}
              />
              {(duplicateRoleKey || invalidRoleKey) && (
                <p className="text-xs text-destructive">
                  {duplicateRoleKey
                    ? t("duplicateRoleKey")
                    : t("invalidRoleKey")}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>{t("agentType")}</Label>
              <Select
                value={agentType}
                onValueChange={(value) => {
                  setAgentType(value as AgentType)
                  setPresetId(CUSTOM_PRESET_ID)
                }}
                disabled={creating}
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
            <Label htmlFor="add-project-role-instructions">
              {t("instructions")}
            </Label>
            <Textarea
              id="add-project-role-instructions"
              value={instructions}
              onChange={(event) => {
                setInstructions(event.target.value)
                setPresetId(CUSTOM_PRESET_ID)
              }}
              placeholder={t("instructionsPlaceholder")}
              disabled={creating}
              className="min-h-24 resize-y"
            />
          </div>

          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={creating}
            type="button"
          >
            {t("cancel")}
          </Button>
          <Button
            onClick={handleCreate}
            disabled={!canCreate || creating}
            type="button"
          >
            {creating && <Loader2 className="mr-2 size-4 animate-spin" />}
            {creating ? t("creating") : t("create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
