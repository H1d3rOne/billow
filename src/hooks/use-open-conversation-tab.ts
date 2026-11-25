"use client"

import { useCallback } from "react"

import { useAppWorkspace } from "@/contexts/app-workspace-context"
import { useTabContext } from "@/contexts/tab-context"
import { listAllConversations } from "@/lib/api"
import type { AgentType, DbConversationSummary } from "@/lib/types"

function deriveTabTitleFromRole(title: string | null | undefined) {
  // Drop common list / heading prefixes from delegated task prompts so the tab
  // reads like a responsibility label instead of a raw prompt fragment.
  return title
    ?.split(/\r?\n/)
    .map((line) => line.trim())
    .map((line) =>
      line
        .replace(/^(?:[-*+]\s+|\d+[.)]\s+|#{1,6}\s+)/, "")
        .replace(/^(?:职责|角色|任务|role|task)\s*[:：]\s*/i, "")
        .trim()
    )
    .find(Boolean)
}

export function useOpenConversationTab() {
  const { conversations, folders, addFolderToWorkspaceById } = useAppWorkspace()
  const { openTab, replaceConversationAgentTab } = useTabContext()

  return useCallback(
    async (
      conversationId: number,
      options?: {
        placement?: "end" | "afterActive"
        title?: string | null
        folderId?: number | null
        agentType?: AgentType | null
        replaceAgentTab?: boolean
      }
    ): Promise<DbConversationSummary> => {
      const requestedAgent = options?.agentType ?? null
      const requestedFolderId = options?.folderId ?? null
      const matchesRequestedShape = (item: DbConversationSummary) =>
        item.id === conversationId &&
        (requestedAgent == null || item.agent_type === requestedAgent) &&
        (requestedFolderId == null || item.folder_id === requestedFolderId)

      let conversation =
        conversations.find(matchesRequestedShape) ??
        conversations.find((item) => item.id === conversationId)

      if (
        !conversation ||
        (requestedAgent != null &&
          conversation.agent_type !== requestedAgent) ||
        (requestedFolderId != null &&
          conversation.folder_id !== requestedFolderId)
      ) {
        const fresh = await listAllConversations({
          include_children: true,
        })
        conversation =
          fresh.find(matchesRequestedShape) ??
          fresh.find((item) => item.id === conversationId) ??
          conversation
      }

      if (!conversation) {
        throw new Error(`Conversation ${conversationId} not found`)
      }

      const folderId = requestedFolderId ?? conversation.folder_id
      const agentType = requestedAgent ?? conversation.agent_type

      if (!folders.some((folder) => folder.id === folderId)) {
        await addFolderToWorkspaceById(folderId)
      }

      const roleTitle = deriveTabTitleFromRole(options?.title)
      const tabTitle = roleTitle ?? conversation.title ?? undefined
      const tabOptions = {
        placement: options?.placement ?? "afterActive",
        titleOverride: roleTitle != null,
      }

      if (options?.replaceAgentTab) {
        replaceConversationAgentTab(
          folderId,
          conversation.id,
          agentType,
          tabTitle,
          tabOptions
        )
      } else {
        openTab(
          folderId,
          conversation.id,
          agentType,
          true,
          tabTitle,
          tabOptions
        )
      }

      return {
        ...conversation,
        folder_id: folderId,
        agent_type: agentType,
      }
    },
    [
      addFolderToWorkspaceById,
      conversations,
      folders,
      openTab,
      replaceConversationAgentTab,
    ]
  )
}
