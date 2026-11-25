# Shared Space（共享空间）设计与实现方案

## 背景

Billow 已经支持文件夹会话、协作项目、委托智能体和不同 Agent 的原生会话。共享空间用于解决另一个问题：同一个工作目录下，用户希望在 Claude Code、Codex CLI、Gemini CLI、OpenCode 等 Agent 之间切换，但**开发上下文仍围绕同一个 Billow 会话延续**。

共享空间不是替换协作项目，也不是把所有 Agent 的原生 session 文件强行改成同一种格式。它在 Billow 内增加一层稳定的数据模型：共享空间包含多个共享会话；每个共享会话可以被多个 Agent 接力操作；每个 Agent 仍保留自己的原生 session/native id。

## 目标

1. 用户可以创建共享空间，选择名称、工作目录和初始 Agent。
2. 共享空间出现在侧边栏“共享空间”分类下。
3. 一个共享空间可以包含多个共享会话。
4. 一个共享会话只有一个稳定的 Billow `conversation_id`，但可以记录多个 Agent 的原生 session。
5. 切换 Agent 时不再为该 Agent 新建一条 Billow conversation，而是继续当前共享会话的同一个 `conversation_id`。
6. 每个 Agent 的 `native_session_id` 分开保存，切回同一 Agent 时优先恢复它之前的原生 session。
7. 共享上下文由后端隐藏注入到 Agent 请求中，不显示在输入框草稿里。
8. 创建新共享会话、切换共享会话、切换 Agent 都应刷新侧边栏和会话 Tab，避免出现同一个共享会话按不同 Agent 重复打开的 Tab。

## 非目标

- 不修改 Claude/Codex/Gemini/OpenCode 等 Agent 的原生 session 存储格式。
- 不把协作项目的“角色/项目经理”模型迁移成共享空间；两者并列存在。
- 不承诺把一个 Agent 内部上下文窗口完整复制到另一个 Agent 私有进程；跨 Agent 延续通过 Billow 数据库、共享上下文快照、原生 session id 映射和后端隐藏注入完成。
- 不把上下文提示词写入用户输入框；提示词只能作为后端隐藏上下文载体，不能作为 UI 可见的伪功能。

## Tutti 可借鉴点

Tutti 的核心价值是把 workspace 作为协作边界：任务、上下文、消息流和 Agent 调度都挂在 workspace 上，而不是挂在某个单一 Agent 上。Billow 共享空间借鉴这一点，但进一步把“会话”作为操作核心：

- 共享空间是工作目录边界。
- 共享会话是开发连续性的核心。
- Agent 是围绕当前共享会话工作的执行端。
- Billow 保存跨 Agent 可读的状态，而不是只依赖 Agent 私有历史。

## 当前数据模型

```text
Shared Space
├── folder_id / working_dir
├── active_shared_conversation_id
├── Shared Conversation A                # 一个稳定 Billow conversation
│   ├── conversation_id
│   ├── active_agent_type
│   ├── Agent Session: Claude Code        # Claude 的 native_session_id
│   ├── Agent Session: Codex CLI          # Codex 的 native_session_id
│   └── Agent Session: Gemini CLI         # Gemini 的 native_session_id
├── Shared Conversation B
│   └── ...
├── Shared Message Timeline               # 共享消息/交接记录
└── Context Snapshot                       # 后端生成的共享上下文快照
```

### 表结构

- `shared_space`
  - `name`
  - `folder_id`
  - `active_shared_conversation_id`
  - `active_agent_binding_id`（旧模型兼容字段，保留但不作为新流程主路径）
  - `created_at` / `updated_at` / `deleted_at`
- `shared_space_conversation`
  - `space_id`
  - `conversation_id`
  - `title`
  - `active_agent_type`
  - `created_at` / `updated_at` / `deleted_at`
- `shared_space_conversation_agent`
  - `shared_conversation_id`
  - `agent_type`
  - `native_session_id`
  - `last_used_at`
  - `created_at` / `updated_at`
- `shared_space_message`
  - `space_id`
  - `binding_id`（旧绑定兼容）
  - `role`
  - `content`
  - `source_agent_type`
  - `native_message_id`
  - `created_at`
- `shared_space_context_snapshot`
  - `space_id`
  - `summary`
  - `recent_messages_json`
  - `updated_at`

## Agent 切换流程

1. 用户在某个共享会话上选择目标 Agent。
2. 后端定位 `shared_space_conversation`，得到稳定的 Billow `conversation_id`。
3. 后端先记住当前 conversation 的 `agent_type + external_id`，写入 `shared_space_conversation_agent`。
4. 后端查找目标 Agent 在该共享会话下的 `native_session_id`。
5. 后端更新同一个 Billow conversation 的：
   - `agent_type = 目标 Agent`
   - `external_id = 目标 Agent 的 native_session_id`（没有则为 `null`，首次发送时由 Agent 启动后回填）
6. 后端更新共享会话的 `active_agent_type` 和共享空间的 `active_shared_conversation_id`。
7. 前端关闭该 `conversation_id` 旧 Agent 维度下可能残留的 Tab，再打开同一个 `conversation_id`。
8. 发送消息时，后端读取共享空间、当前共享会话和各 Agent 最近原生 session 记录，作为隐藏 `Resource` 注入给 Agent。

## 当前落地实现（2026-07-07）

### 代码入口

- 数据库实体与迁移
  - `src-tauri/src/db/entities/shared_space.rs`
  - `src-tauri/src/db/entities/shared_space_conversation.rs`
  - `src-tauri/src/db/entities/shared_space_conversation_agent.rs`
  - `src-tauri/src/db/entities/shared_space_message.rs`
  - `src-tauri/src/db/entities/shared_space_context_snapshot.rs`
  - `src-tauri/src/db/migration/m20260707_000002_shared_space_conversation.rs`
- 数据服务
  - `src-tauri/src/db/service/shared_space_service.rs`
- 后端命令 / Web handler
  - `src-tauri/src/commands/shared_spaces.rs`
  - `src-tauri/src/web/handlers/shared_spaces.rs`
  - `src-tauri/src/web/router.rs`
- ACP 注入与发送
  - `src-tauri/src/acp/manager.rs`
  - `src-tauri/src/acp/connection.rs`
- 会话读取与跨 Agent 历史合并
  - `src-tauri/src/commands/conversations.rs`
  - `src-tauri/src/parsers/mod.rs`
  - `src-tauri/src/parsers/claude.rs`
- 前端 API / 类型
  - `src/lib/api.ts`
  - `src/lib/types.ts`
  - `src/components/shared-spaces/*`
  - `src/components/conversations/sidebar-conversation-*`

### 对外命令/API

Tauri command 和 server mode HTTP API 共用同一组 core 函数：

- `create_shared_space`
  - 创建/打开工作目录对应的 `folder`
  - 创建 `shared_space`
  - 自动创建第一条 Billow conversation
  - 创建第一条 `shared_space_conversation`
  - 初始化当前 Agent 的 `shared_space_conversation_agent`
  - 生成初始 context snapshot
- `list_shared_spaces`
  - 返回空间、工作目录、活跃共享会话、共享会话列表、每个共享会话下已使用 Agent session。
- `create_shared_space_conversation`
  - 在同一个空间下新建共享会话。
  - 每条共享会话有独立稳定的 Billow `conversation_id`。
- `switch_shared_space_agent`
  - 在同一个共享会话内切换执行 Agent。
  - 不创建新的 Billow conversation。
  - 更新同一条 conversation 的 `agent_type` 和 `external_id`。
- `remove_shared_space`
  - 软删除共享空间。
- `append_shared_space_message` / `list_shared_space_messages`
  - 预留给共享空间时间线、交接记录和长期记忆。

### 稳定会话 ID 策略

共享空间的关键约束是：**Billow UI 面向稳定的 `conversation_id`，Agent 运行时面向各自的 `native_session_id`**。

```text
shared_space_conversation.conversation_id
  = Billow 层稳定会话 ID，Tab、会话详情和侧边栏都围绕它工作

shared_space_conversation_agent.native_session_id
  = 每个 Agent 私有原生 session ID
  = Claude/Codex/Gemini/OpenCode/Cline/OpenClaw/Hermes 各自 parser 能读取的 ID
```

切换 Agent 时：

1. `remember_current_native_session()` 先把当前 conversation 的 `external_id` 保存回当前 Agent 的 session 记录。
2. `ensure_conversation_agent_session()` 确保目标 Agent 在当前共享会话下有一条 session 记录。
3. `set_shared_conversation_agent()` 更新当前共享会话的 `active_agent_type`。
4. `conversation_service::update_agent_type_and_external_id()` 把同一条 Billow conversation 切换到目标 Agent 和目标 `native_session_id`。
5. `emit_conversation_upsert()` 通知前端刷新侧边栏/会话详情。

### 隐藏上下文注入

共享空间跨 Agent 连续性的核心不是把文字塞进输入框，而是后端在 ACP 发送时注入隐藏资源。

1. `build_shared_space_context_snapshot()` 生成共享上下文：
   - 空间名称、工作目录、目标 Agent
   - 当前空间下所有共享会话
   - 每个共享会话已使用过的 Agent 和 `native_session_id` 摘要
   - 最近 `shared_space_message`
   - 每个共享会话按 Agent 分组的最近原生 transcript turns
   - 接手要求：继续当前共享会话、先确认运行状态、以当前工作目录真实状态为准、重要交接写回共享空间
2. 生成结果写入 `shared_space_context_snapshot`：
   - `summary`：截断到 8000 字符以内的 Markdown 快照
   - `recent_messages_json`：结构化最近会话摘录
3. `build_shared_space_hidden_context_blocks()` 把快照包装成 ACP resource：

```text
billow://shared-space/{space_id}/conversation/{shared_conversation_id}/context.md
```

4. `send_prompt_linked_with_message_id()` 只在普通用户发送、非 delegation 子任务时构造 `hidden_context_blocks`。
5. `connection.rs` 在真正发给 Agent 前把 `hidden_context_blocks` prepend 到用户真实输入 blocks 前面。
6. `user_message` 和前端乐观消息只包含用户真实输入，不包含隐藏 context。

### 隐藏上下文的显示层清洗

部分 Agent/SDK 会把 ACP resource 持久化回自己的 transcript，变成普通用户文本，例如：

```text
billow://shared-space/2/conversation/2/context.md

<context ref="billow://shared-space/2/conversation/2/context.md">
...
</context>
```

这类内容是 Billow 给 Agent 的隐藏接手上下文，不是用户发言。为避免重新读取历史时污染 UI：

- `src-tauri/src/parsers/mod.rs`
  - `strip_hidden_shared_space_context_text()`
  - `strip_hidden_shared_space_context_blocks()`
  - `strip_hidden_shared_space_context_turns()`
- `src-tauri/src/parsers/claude.rs`
  - `strip_user_invisible_text()` 同时清理 Claude 系统标签和共享空间隐藏上下文。
  - `extract_user_text()`、`extract_user_content()`、`parse_conversation_detail()` 都会走清洗逻辑。
- `src-tauri/src/commands/conversations.rs`
  - 主会话读取后清洗一次。
  - 合并共享空间其它 Agent session 历史前再清洗一次。

原则：**不要删除隐藏上下文注入能力，只在解析/展示层过滤 Agent 持久化出来的副本**。

### 跨 Agent 历史合并

共享会话详情页需要展示同一个共享会话中多个 Agent 的历史。实现位置在 `get_folder_conversation_core()`：

1. 先按当前 conversation 的 `agent_type + external_id` 读取当前 Agent transcript。
2. 判断该 conversation 是否属于某个 `shared_space_conversation`。
3. 如果属于共享会话，读取 `shared_space_conversation_agent` 中其它 Agent 的 `native_session_id`。
4. 对每个非当前 Agent session 调用对应 parser 读取 turns。
5. 对读出的 turns 做隐藏上下文清洗。
6. 给共享空间合并进来的 turn id 加前缀：

```text
shared-space:{agent_type}:{native_session_id}:{original_turn_id}
```

7. 合并后按 `timestamp` 排序。

注意点：

- 当前 Agent 已成功读取时，按 `agent_type` 跳过同类 session，避免重复渲染。
- OpenClaw/Cline/Gemini 等外部 id 可能漂移，读取失败时会按 `folder_path + created_at` 做窄范围 fallback。
- 合并历史只影响 Billow 展示，不修改 Agent 原生 transcript。

## UI 规划

侧边栏新增“共享空间”：

```text
共享空间
├── 项目 A                         (右键：打开当前会话 / 新建共享会话 / 切换智能体 / 打开工作目录 / 从工作区移除)
│   ├── 共享空间 · 项目 A          (右键：打开会话 / 切换智能体)
│   └── 共享空间 · 项目 A · 会话2
└── 项目 B
```

关键交互：

- “+” 创建共享空间，并自动创建第一个共享会话。
- 共享空间右键可以新建共享会话。
- 展开共享空间后展示共享会话，而不是展示旧的 Agent 绑定。
- 共享会话右键可以选择任意 Agent 来接力当前会话。
- 切换 Agent 后仍打开同一个 Billow conversation，只是当前执行 Agent 变更。

## 分阶段实现

### Phase 1：会话核心闭环

- 新增 `shared_space_conversation` 和 `shared_space_conversation_agent`。
- 创建共享空间时自动创建第一个共享会话。
- 支持在共享空间下新增多个共享会话。
- 支持同一共享会话在多个 Agent 间切换并保存各自原生 session id。
- 共享空间侧边栏展示“空间 → 共享会话”。
- 后端隐藏注入共享上下文，不写入输入框。

### Phase 2：统一时间线

- 在发送/接收消息时写入 `shared_space_message`。
- 支持共享空间详情页展示跨 Agent 时间线。
- 根据最近消息持续更新 `shared_space_context_snapshot`。

### Phase 3：工具层集成

- 通过 MCP 暴露共享空间上下文读取/写入工具。
- Agent 可以主动读取共享空间摘要、记录决策、追加交接信息。
- 支持长期共享空间中多 Agent 接力，并保持 Billow 级别项目记忆。

## 与协作项目的关系

- 协作项目：面向多角色并行协作，项目经理主导，角色通常长期固定。
- 共享空间：面向单一项目/工作目录下的多 Agent 切换，核心是“共享会话连续性”。

两者可以组合：未来可以让协作项目中的某个角色会话绑定到共享空间的共享会话，但当前实现不耦合两套功能，避免影响已有委托/项目角色逻辑。
