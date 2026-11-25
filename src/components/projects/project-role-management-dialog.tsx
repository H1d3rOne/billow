"use client"

import { useState } from "react"
import { useTranslations } from "next-intl"
import { Loader2, Plus, RotateCcw, Wrench } from "lucide-react"

import { AgentIcon } from "@/components/agent-icon"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  AGENT_LABELS,
  type CollaborativeProjectRoleSummary,
  type CollaborativeProjectSummary,
} from "@/lib/types"
import { cn } from "@/lib/utils"

interface ProjectRoleManagementDialogProps {
  project: CollaborativeProjectSummary | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onAddRole: (project: CollaborativeProjectSummary) => void
  onOpenRole: (
    project: CollaborativeProjectSummary,
    role: CollaborativeProjectRoleSummary
  ) => Promise<void> | void
  onEnsureRoleConversation: (
    project: CollaborativeProjectSummary,
    role: CollaborativeProjectRoleSummary
  ) => Promise<void> | void
  onRecreateRoleConversation: (
    project: CollaborativeProjectSummary,
    role: CollaborativeProjectRoleSummary
  ) => Promise<void> | void
}

export function ProjectRoleManagementDialog({
  project,
  open,
  onOpenChange,
  onAddRole,
  onOpenRole,
  onEnsureRoleConversation,
  onRecreateRoleConversation,
}: ProjectRoleManagementDialogProps) {
  const t = useTranslations("CollaborativeProject.roleManagementDialog")
  const [pending, setPending] = useState<string | null>(null)
  const activeProject = project

  async function run(
    key: string,
    action: () => Promise<void> | void
  ): Promise<void> {
    if (pending) return
    setPending(key)
    try {
      await action()
    } finally {
      setPending(null)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="grid max-h-[86vh] grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{t("title")}</DialogTitle>
          <DialogDescription>
            {t("description", { name: project?.name ?? "" })}
          </DialogDescription>
        </DialogHeader>

        <ScrollArea className="min-h-0 pr-3">
          <div className="space-y-2 py-1">
            {activeProject
              ? activeProject.roles.map((role) => {
                  const missing = role.conversationId == null
                  const rowPending = pending?.startsWith(`${role.roleId}:`)
                  return (
                    <div
                      key={role.roleId}
                      className={cn(
                        "rounded-2xl border bg-card/50 p-3 shadow-sm",
                        role.isManager && "border-primary/30 bg-primary/5"
                      )}
                    >
                      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                        <div className="min-w-0 space-y-1">
                          <div className="flex min-w-0 flex-wrap items-center gap-2">
                            <span className="truncate text-sm font-medium">
                              {role.name}
                            </span>
                            {role.isManager ? (
                              <span className="rounded-full bg-primary/10 px-2 py-0.5 text-[0.6875rem] font-medium text-primary">
                                {t("managerBadge")}
                              </span>
                            ) : null}
                            <span className="rounded-full bg-muted px-2 py-0.5 text-[0.6875rem] text-muted-foreground">
                              {role.roleKey}
                            </span>
                          </div>
                          <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
                            <span className="inline-flex items-center gap-1.5">
                              <AgentIcon
                                agentType={role.agentType}
                                className="size-3.5"
                              />
                              {AGENT_LABELS[role.agentType]}
                            </span>
                            <span
                              className={cn(
                                "truncate",
                                missing && "text-destructive"
                              )}
                              title={
                                missing
                                  ? t("missingConversation")
                                  : (role.conversationTitle ?? undefined)
                              }
                            >
                              {missing
                                ? t("missingConversation")
                                : (role.conversationTitle ?? t("untitled"))}
                            </span>
                          </div>
                        </div>

                        <div className="flex shrink-0 flex-wrap items-center gap-2">
                          <Button
                            size="sm"
                            variant={missing ? "default" : "outline"}
                            disabled={rowPending}
                            onClick={() =>
                              run(`${role.roleId}:open`, () =>
                                missing
                                  ? onEnsureRoleConversation(
                                      activeProject,
                                      role
                                    )
                                  : onOpenRole(activeProject, role)
                              )
                            }
                          >
                            {pending === `${role.roleId}:open` ? (
                              <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                            ) : missing ? (
                              <Wrench className="mr-1.5 size-3.5" />
                            ) : null}
                            {missing ? t("repair") : t("open")}
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={rowPending}
                            onClick={() =>
                              run(`${role.roleId}:recreate`, () =>
                                onRecreateRoleConversation(activeProject, role)
                              )
                            }
                          >
                            {pending === `${role.roleId}:recreate` ? (
                              <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                            ) : (
                              <RotateCcw className="mr-1.5 size-3.5" />
                            )}
                            {t("recreate")}
                          </Button>
                        </div>
                      </div>
                    </div>
                  )
                })
              : null}
          </div>
        </ScrollArea>

        <DialogFooter className="gap-2 sm:justify-between">
          <Button
            variant="outline"
            type="button"
            disabled={!project}
            onClick={() => project && onAddRole(project)}
          >
            <Plus className="mr-1.5 size-3.5" />
            {t("addRole")}
          </Button>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
