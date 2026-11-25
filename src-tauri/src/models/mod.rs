pub mod agent;
pub mod chat_channel;
pub mod conversation;
pub mod folder;
pub mod message;
pub mod model_provider;
pub mod pet;
pub mod project;
pub mod quick_message;
pub mod remote_workspace_connection;
pub mod shared_space;
pub mod system;

pub use agent::AgentType;
#[allow(unused_imports)]
pub use chat_channel::{ChannelStatusInfo, ChatChannelInfo, ChatChannelMessageLogInfo};
pub use conversation::{
    AgentConversationCount, AgentStats, ConversationDetail, ConversationSummary,
    DbConversationDetail, DbConversationSummary, FolderInfo, ImportResult, SessionStats,
    SidebarData,
};
pub use folder::{
    FolderCommandInfo, FolderDetail, FolderHistoryEntry, OpenedTab, OpenedTabsSnapshot,
    SaveTabsOutcome,
};
pub use message::{
    AgentExecutionStats, AgentToolCall, ContentBlock, ImageData, MessageRole, MessageTurn,
    TurnRole, TurnUsage, UnifiedMessage,
};
pub use project::{
    AddCollaborativeProjectRoleParams, AddCollaborativeProjectRoleResult,
    CollaborativeProjectRoleSummary, CollaborativeProjectSummary, CreateCollaborativeProjectParams,
    CreateCollaborativeProjectResult, CreateCollaborativeProjectRoleParams,
    EnsureProjectRoleConversationParams, ProjectRoleConversationResult,
    RemoveCollaborativeProjectResult,
};
pub use quick_message::QuickMessageInfo;
pub use remote_workspace_connection::RemoteWorkspaceConnectionInfo;
pub use shared_space::{
    AppendSharedSpaceMessageParams, CreateSharedSpaceConversationParams,
    CreateSharedSpaceConversationResult, CreateSharedSpaceParams, CreateSharedSpaceResult,
    ListSharedSpaceMessagesParams, RemoveSharedSpaceResult, SharedSpaceConversationAgentSummary,
    SharedSpaceConversationSummary, SharedSpaceMessageSummary, SharedSpaceSummary,
    SwitchSharedSpaceAgentParams, SwitchSharedSpaceAgentResult,
};
#[cfg(feature = "tauri-runtime")]
pub use system::SystemRenderingSettings;
pub use system::{
    AvailableTerminalShells, GitCredentials, GitDetectResult, GitHubAccountsSettings,
    GitHubTokenValidation, GitSettings, SystemLanguageSettings, SystemProxySettings,
    SystemTerminalSettings, TerminalShellOption,
};
