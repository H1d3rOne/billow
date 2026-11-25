"use client"

import { useMemo, useState } from "react"
import { useTranslations } from "next-intl"
import { toast } from "sonner"
import { FolderOpen, Loader2 } from "lucide-react"

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
import { useAppWorkspace } from "@/contexts/app-workspace-context"
import { useOpenConversationTab } from "@/hooks/use-open-conversation-tab"
import { createSharedSpace } from "@/lib/api"
import { toErrorMessage } from "@/lib/app-error"
import { isDesktop, openFileDialog } from "@/lib/platform"
import { getActiveRemoteConnectionId } from "@/lib/transport"
import { AGENT_LABELS, ALL_AGENT_TYPES, type AgentType } from "@/lib/types"

interface CreateSharedSpaceDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function CreateSharedSpaceDialog({
  open,
  onOpenChange,
}: CreateSharedSpaceDialogProps) {
  const t = useTranslations("SharedSpace.createDialog")
  const { upsertFolder, refreshConversations, refreshSharedSpaces } =
    useAppWorkspace()
  const openConversationTab = useOpenConversationTab()

  const [name, setName] = useState("")
  const [workingDir, setWorkingDir] = useState("")
  const [agentType, setAgentType] = useState<AgentType>("claude_code")
  const [creating, setCreating] = useState(false)
  const [browserOpen, setBrowserOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const canCreate = useMemo(
    () => name.trim().length > 0 && workingDir.trim().length > 0,
    [name, workingDir]
  )

  function resetForm() {
    setName("")
    setWorkingDir("")
    setAgentType("claude_code")
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

  async function handleCreate() {
    if (!canCreate || creating) return
    setCreating(true)
    setError(null)
    try {
      const result = await createSharedSpace({
        name: name.trim(),
        workingDir: workingDir.trim(),
        agentType,
      })
      upsertFolder(result.folder)
      await refreshConversations()
      await refreshSharedSpaces()
      await openConversationTab(result.activeConversationId, {
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
    <>
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
            <DialogDescription>{t("description")}</DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-1">
            <div className="space-y-2">
              <Label htmlFor="shared-space-name">{t("name")}</Label>
              <Input
                id="shared-space-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder={t("namePlaceholder")}
                disabled={creating}
                autoFocus
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="shared-space-working-dir">
                {t("workingDir")}
              </Label>
              <div className="flex gap-2">
                <Input
                  id="shared-space-working-dir"
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

            <div className="space-y-2">
              <Label>{t("agentType")}</Label>
              <Select
                value={agentType}
                onValueChange={(value) => setAgentType(value as AgentType)}
                disabled={creating}
              >
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {ALL_AGENT_TYPES.map((agent) => (
                    <SelectItem key={agent} value={agent}>
                      {AGENT_LABELS[agent]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground">
                {t("agentTypeHint")}
              </p>
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
