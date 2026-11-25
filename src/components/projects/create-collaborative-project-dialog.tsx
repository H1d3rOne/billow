"use client"

import { useEffect, useMemo, useState } from "react"
import { useTranslations } from "next-intl"
import { toast } from "sonner"
import { FolderOpen, Loader2, Plus, Trash2 } from "lucide-react"

import { DirectoryBrowserDialog } from "@/components/shared/directory-browser-dialog"
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
import { Separator } from "@/components/ui/separator"
import { Textarea } from "@/components/ui/textarea"
import { useAppWorkspace } from "@/contexts/app-workspace-context"
import { useOpenConversationTab } from "@/hooks/use-open-conversation-tab"
import { createCollaborativeProject } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import {
  buildBuiltInRolePresets,
  makeId,
  makeUniqueRoleKey,
  readUserRolePresets,
  slugifyRoleKey,
  subscribeUserRolePresets,
  type CollaborativeRolePreset,
} from "@/lib/collaborative-role-presets"
import { isDesktop, openFileDialog } from "@/lib/platform"
import { getActiveRemoteConnectionId } from "@/lib/transport"
import { AGENT_LABELS, type AgentType } from "@/lib/types"
import { cn } from "@/lib/utils"

const CUSTOM_PRESET_ID = "__custom__"

interface RoleDraft {
  id: string
  name: string
  roleKey: string
  agentType: AgentType
  instructions: string
  isManager: boolean
  presetId?: string
}

interface CreateCollaborativeProjectDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

const AGENT_OPTIONS: AgentType[] = [
  "claude_code",
  "codex",
  "open_code",
  "gemini",
  "open_claw",
  "cline",
  "hermes",
]

interface DefaultRoleLabels {
  managerName: string
  managerInstructions: string
}

function defaultRoles(labels: DefaultRoleLabels): RoleDraft[] {
  return [
    {
      id: makeId(),
      name: labels.managerName,
      roleKey: "project_manager",
      agentType: "claude_code",
      instructions: labels.managerInstructions,
      isManager: true,
      presetId: "builtin:project_manager",
    },
  ]
}

export function CreateCollaborativeProjectDialog({
  open,
  onOpenChange,
}: CreateCollaborativeProjectDialogProps) {
  const t = useTranslations("CollaborativeProject.createDialog")
  const { upsertFolder, refreshConversations, refreshProjects } =
    useAppWorkspace()
  const openConversationTab = useOpenConversationTab()
  const defaultRoleLabels: DefaultRoleLabels = {
    managerName: t("roles.manager.name"),
    managerInstructions: t("roles.manager.instructions"),
  }

  const [projectName, setProjectName] = useState("")
  const [workingDir, setWorkingDir] = useState("")
  const [roles, setRoles] = useState<RoleDraft[]>(() =>
    defaultRoles(defaultRoleLabels)
  )
  const [userPresets, setUserPresets] = useState<CollaborativeRolePreset[]>([])
  const [creating, setCreating] = useState(false)
  const [browserOpen, setBrowserOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const builtInPresets = useMemo(
    () =>
      buildBuiltInRolePresets({
        managerName: t("roles.manager.name"),
        managerInstructions: t("roles.manager.instructions"),
        frontendName: t("roles.frontend.name"),
        frontendInstructions: t("roles.frontend.instructions"),
        backendName: t("roles.backend.name"),
        backendInstructions: t("roles.backend.instructions"),
        qaName: t("roles.qa.name"),
        qaInstructions: t("roles.qa.instructions"),
      }),
    [t]
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

  const roleKeyCounts = useMemo(() => {
    const counts = new Map<string, number>()
    for (const role of roles) {
      const key = role.roleKey.trim()
      if (!key) continue
      counts.set(key, (counts.get(key) ?? 0) + 1)
    }
    return counts
  }, [roles])

  const canCreate =
    projectName.trim().length > 0 &&
    workingDir.trim().length > 0 &&
    roles.length > 0 &&
    roles.some((role) => role.isManager) &&
    roles.every(
      (role) =>
        role.name.trim() &&
        role.roleKey.trim() &&
        /^[a-z0-9_-]+$/.test(role.roleKey.trim()) &&
        (roleKeyCounts.get(role.roleKey.trim()) ?? 0) === 1
    )

  function resetForm() {
    setProjectName("")
    setWorkingDir("")
    setRoles(defaultRoles(defaultRoleLabels))
    setError(null)
  }

  async function handleBrowse() {
    if (isDesktop() && getActiveRemoteConnectionId() === null) {
      const selected = await openFileDialog({
        directory: true,
        multiple: false,
      })
      if (!selected) return
      setWorkingDir(Array.isArray(selected) ? selected[0] : selected)
    } else {
      setBrowserOpen(true)
    }
  }

  function patchRole(id: string, patch: Partial<RoleDraft>) {
    setRoles((prev) =>
      prev.map((role) => (role.id === id ? { ...role, ...patch } : role))
    )
  }

  function handleRoleNameChange(role: RoleDraft, name: string) {
    const next: Partial<RoleDraft> = { name, presetId: CUSTOM_PRESET_ID }
    const generated = slugifyRoleKey(role.name)
    if (!role.roleKey || role.roleKey === generated) {
      next.roleKey = slugifyRoleKey(name)
    }
    patchRole(role.id, next)
  }

  function addRole() {
    const base = t("roles.custom.name")
    const existing = new Set(roles.map((role) => role.roleKey))
    const preset = rolePresets.find((item) => !existing.has(item.roleKey))
    const roleKey = preset
      ? makeUniqueRoleKey(preset.roleKey, existing)
      : makeUniqueRoleKey(`custom_${roles.length + 1}`, existing)
    setRoles((prev) => [
      ...prev,
      {
        id: makeId(),
        name: preset?.name ?? base,
        roleKey,
        agentType: preset?.agentType ?? "claude_code",
        instructions: preset?.instructions ?? "",
        isManager: false,
        presetId: preset?.id ?? CUSTOM_PRESET_ID,
      },
    ])
  }

  function applyRolePreset(role: RoleDraft, nextPresetId: string) {
    if (nextPresetId === CUSTOM_PRESET_ID) {
      patchRole(role.id, { presetId: CUSTOM_PRESET_ID })
      return
    }
    const preset = rolePresets.find((item) => item.id === nextPresetId)
    if (!preset) return
    const existing = new Set(
      roles
        .filter((item) => item.id !== role.id)
        .map((item) => item.roleKey.trim())
    )
    patchRole(role.id, {
      presetId: preset.id,
      name: preset.name,
      roleKey: makeUniqueRoleKey(preset.roleKey, existing),
      agentType: preset.agentType,
      instructions: preset.instructions,
    })
  }

  function removeRole(id: string) {
    setRoles((prev) => prev.filter((role) => role.id !== id || role.isManager))
  }

  async function handleCreate() {
    if (!canCreate || creating) return
    setCreating(true)
    setError(null)
    try {
      const result = await createCollaborativeProject({
        name: projectName.trim(),
        workingDir: workingDir.trim(),
        roles: roles.map((role) => ({
          name: role.name.trim(),
          roleKey: role.roleKey.trim(),
          agentType: role.agentType,
          instructions: role.instructions.trim() || null,
          isManager: role.isManager,
        })),
      })
      upsertFolder(result.folder)
      await refreshConversations()
      await refreshProjects()
      await openConversationTab(result.managerConversationId, {
        placement: "afterActive",
        title: result.managerTitle,
      })
      toast.success(t("toasts.created"), {
        description: result.managerTitle,
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
    <>
      <Dialog
        open={open}
        onOpenChange={(value) => {
          onOpenChange(value)
          if (!value) resetForm()
        }}
      >
        <DialogContent className="grid max-h-[88vh] grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden sm:max-w-3xl">
          <DialogHeader>
            <DialogTitle>{t("title")}</DialogTitle>
            <DialogDescription>{t("description")}</DialogDescription>
          </DialogHeader>

          <div className="min-h-0 space-y-5 overflow-y-auto py-1 pr-1">
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="collab-project-name">{t("projectName")}</Label>
                <Input
                  id="collab-project-name"
                  value={projectName}
                  onChange={(event) => setProjectName(event.target.value)}
                  placeholder={t("projectNamePlaceholder")}
                  disabled={creating}
                  autoFocus
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="collab-working-dir">{t("workingDir")}</Label>
                <div className="flex gap-2">
                  <Input
                    id="collab-working-dir"
                    value={workingDir}
                    onChange={(event) => setWorkingDir(event.target.value)}
                    placeholder={t("workingDirPlaceholder")}
                    disabled={creating}
                    className="min-w-0 flex-1"
                  />
                  <Button
                    variant="outline"
                    size="icon"
                    onClick={handleBrowse}
                    disabled={creating}
                    title={t("browse")}
                    aria-label={t("browse")}
                    type="button"
                  >
                    <FolderOpen className="size-4" />
                  </Button>
                </div>
              </div>
            </div>

            <Separator />

            <div className="space-y-3">
              <div className="flex items-center justify-between gap-3">
                <div>
                  <Label>{t("rolesTitle")}</Label>
                  <p className="text-xs text-muted-foreground">
                    {t("rolesDescription")}
                  </p>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={addRole}
                  disabled={creating}
                  type="button"
                >
                  <Plus className="size-3.5" />
                  {t("addRole")}
                </Button>
              </div>

              <div className="max-h-[48vh] space-y-3 overflow-y-auto pr-1">
                {roles.map((role) => {
                  const duplicate =
                    (roleKeyCounts.get(role.roleKey.trim()) ?? 0) > 1
                  const invalidKey =
                    role.roleKey.trim().length > 0 &&
                    !/^[a-z0-9_-]+$/.test(role.roleKey.trim())
                  return (
                    <div
                      key={role.id}
                      className={cn(
                        "rounded-2xl border bg-card/50 p-3 shadow-sm",
                        role.isManager && "border-primary/30 bg-primary/5"
                      )}
                    >
                      <div className="mb-3 flex items-center justify-between gap-2">
                        <div className="text-sm font-medium">
                          {role.isManager ? t("managerBadge") : t("roleBadge")}
                        </div>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => removeRole(role.id)}
                          disabled={creating || role.isManager}
                          title={
                            role.isManager
                              ? t("managerRequired")
                              : t("removeRole")
                          }
                          aria-label={
                            role.isManager
                              ? t("managerRequired")
                              : t("removeRole")
                          }
                          type="button"
                          className="size-7 text-muted-foreground hover:text-destructive"
                        >
                          <Trash2 className="size-3.5" />
                        </Button>
                      </div>

                      {!role.isManager && (
                        <div className="mb-3 space-y-1.5">
                          <Label>{t("rolePreset")}</Label>
                          <Select
                            value={role.presetId ?? CUSTOM_PRESET_ID}
                            onValueChange={(value) =>
                              applyRolePreset(role, value)
                            }
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
                        </div>
                      )}

                      <div className="grid gap-3 sm:grid-cols-[1fr_1fr_180px]">
                        <div className="space-y-1.5">
                          <Label>{t("roleName")}</Label>
                          <Input
                            value={role.name}
                            onChange={(event) =>
                              handleRoleNameChange(role, event.target.value)
                            }
                            disabled={creating}
                          />
                        </div>
                        <div className="space-y-1.5">
                          <Label>{t("roleKey")}</Label>
                          <Input
                            value={role.roleKey}
                            onChange={(event) =>
                              patchRole(role.id, {
                                roleKey: event.target.value
                                  .trim()
                                  .toLowerCase(),
                                presetId: CUSTOM_PRESET_ID,
                              })
                            }
                            disabled={creating}
                            aria-invalid={duplicate || invalidKey}
                          />
                          {(duplicate || invalidKey) && (
                            <p className="text-xs text-destructive">
                              {duplicate
                                ? t("duplicateRoleKey")
                                : t("invalidRoleKey")}
                            </p>
                          )}
                        </div>
                        <div className="space-y-1.5">
                          <Label>{t("agentType")}</Label>
                          <Select
                            value={role.agentType}
                            onValueChange={(value) =>
                              patchRole(role.id, {
                                agentType: value as AgentType,
                                presetId: CUSTOM_PRESET_ID,
                              })
                            }
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

                      <div className="mt-3 space-y-1.5">
                        <Label>{t("instructions")}</Label>
                        <Textarea
                          value={role.instructions}
                          onChange={(event) =>
                            patchRole(role.id, {
                              instructions: event.target.value,
                              presetId: CUSTOM_PRESET_ID,
                            })
                          }
                          disabled={creating}
                          placeholder={t("instructionsPlaceholder")}
                          className="min-h-20 resize-y"
                        />
                      </div>
                    </div>
                  )
                })}
              </div>
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
              {t("create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <DirectoryBrowserDialog
        open={browserOpen}
        onOpenChange={setBrowserOpen}
        onSelect={(path) => setWorkingDir(path)}
        title={t("selectWorkingDir")}
      />
    </>
  )
}
