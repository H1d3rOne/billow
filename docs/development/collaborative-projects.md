# 协作项目：创建项目与角色编排方案

## 背景

Billow 已支持通过 MCP `delegate_to_agent` 进行多智能体委托，并支持按稳定 `role` 复用长期 collaborator 会话。下一步需要在产品层新增“协作项目”能力：用户可以创建一个项目，绑定工作目录，并配置多个项目角色；其中“项目经理”是必选角色，整个项目由项目经理主导规划、拆解任务、委托角色并汇总结果。

该功能不是普通代码模板脚手架，而是 Billow 内部的多智能体协作项目管理能力。

## 目标

创建项目时应支持：

- 选择项目工作目录。
- 配置角色列表。
  - 默认只包含项目经理。
  - 项目经理为必选角色。
  - 用户可以自定义角色，例如前端、后端、测试等。
- 创建后自动建立项目经理会话。
- 项目后续任务默认由项目经理主导。
- 项目角色、项目经理会话、角色长期会话绑定必须落到数据库和工具链路中。
- 协作项目不能靠把一大段提示词预填到输入框来实现；提示词只能作为 agent 行为补充，不是功能本体。

## 非目标

第一阶段不直接实现完整项目管理系统，例如：

- 甘特图、迭代管理、任务看板。
- 独立权限系统。
- 跨设备实时多人协同。
- 完整的项目重启恢复和历史角色会话重建。

这些可以作为后续阶段扩展。

## 核心设计

### 1. 新增 Project 概念

现有 `folder` 表代表工作目录，但“项目”需要保存更多协作语义，因此建议新增 Project 领域模型，而不是只复用 Folder。

建议新增表：

```text
project
- id
- name
- folder_id
- manager_role_id
- manager_conversation_id
- created_at
- updated_at
- deleted_at

project_role
- id
- project_id
- name
- role_key
- agent_type
- instructions
- is_manager
- conversation_id
- sort_order
- created_at
- updated_at
```

字段说明：

- `project.folder_id`：绑定已有工作目录。
- `project.manager_role_id`：指向项目经理角色。
- `project.manager_conversation_id`：项目经理主会话。
- `project_role.role_key`：稳定委托 key，例如 `project_manager`、`frontend`、`backend`、`qa`。
- `project_role.agent_type`：该角色默认使用的智能体类型。
- `project_role.instructions`：角色职责说明。
- `project_role.is_manager`：是否为项目经理。
- `project_role.conversation_id`：该角色对应的长期会话，后续用于重启恢复和角色 tab 复用。

### 2. 项目经理主导项目

创建项目后，应自动创建项目经理 conversation，并打开该会话 tab。

项目经理会话标题建议：

```text
项目经理 - {项目名}
```

项目经理会话只作为项目入口之一；项目上下文应由 Billow 的项目元数据和工具层提供，而不是在输入框中塞入一段“项目经理启动提示词”。

系统应能根据 `project.manager_conversation_id` 找到项目，并在委托链路中读取：

- 项目名称。
- 项目工作目录。
- 项目角色列表。
- 每个角色的职责说明。
- 角色默认 agent 类型。
- `project_id + role_key -> conversation_id` 的长期绑定。

用户进入项目后，默认面向项目经理发起需求。项目经理负责：

1. 理解需求。
2. 澄清问题。
3. 拆解任务。
4. 通过 Billow 的项目委托功能/工具按角色委托。
5. 通过委托状态能力收集结果。
6. 汇总给用户。

### 3. 角色模板

创建项目默认只包含项目经理：

```text
项目经理
- role_key: project_manager
- required: true
- 负责需求澄清、任务拆解、进度协调、结果汇总。
```

用户可以按需添加常见协作角色：

```text
前端
- role_key: frontend
- 负责 UI、交互、前端工程。

后端
- role_key: backend
- 负责 API、数据库、服务逻辑。

测试
- role_key: qa
- 负责测试计划、自动化测试、回归验证。
```

用户可以：

- 修改非必填角色。
- 删除非项目经理角色。
- 新增自定义角色。
- 修改角色名称、智能体类型、职责说明。

项目经理角色不可删除；如果允许重命名，也必须保留 `is_manager = true` 的语义。

### 4. UI 入口与创建流程

建议新增入口：

```text
新建协作项目
```

可以放在：

- 侧边栏新建入口。
- Project Boot 附近。
- 工作区空状态页。

创建弹窗分三块：

1. 项目信息
   - 项目名称。
   - 工作目录选择。

2. 角色配置
   - 默认只有项目经理。
   - 项目经理固定必选。
   - 添加自定义角色按钮。
   - 角色名称、角色 key、agent 类型、职责说明。

3. 创建后行为
   - 创建 project。
   - 创建 / 打开 folder。
   - 创建项目经理 conversation。
   - 打开项目经理 tab。

### 5. 与长期 collaborator 的关系

当前长期 collaborator 支持通过 MCP 参数：

```json
{
  "role": "frontend",
  "reuse": true
}
```

项目功能复用该机制，但不再只靠提示词要求模型“记住角色”。Billow
在 MCP 伴生进程中新增 `list_project_roles`，并在主进程 listener 中根据
`project.manager_conversation_id` 读取数据库里的角色配置。项目经理委托时应先读取
可用角色，再使用项目角色的稳定 `role_key`：

```json
{
  "agent_type": "codex",
  "role": "frontend",
  "reuse": true,
  "task": "..."
}
```

主进程会强制执行这些配置：

- 如果当前会话是协作项目经理会话，`delegate_to_agent` 必须传入一个已配置的
  非项目经理 `role_key`。
- 模型传入的 `agent_type`、`working_dir`、`reuse` 会被数据库里的角色配置覆盖。
- `reuse` 强制为 `true`，避免项目角色被一次性会话替代。
- 初次成功委托后，子会话 id 写回 `project_role.conversation_id`。

注意：当前长期 collaborator 主要是运行时复用。项目功能为了支持真正长期协作，需要进一步把角色会话关系持久化：

```text
project_id + role_key -> conversation_id
```

这样即使客户端重启，也能恢复：

- 项目有哪些角色。
- 项目经理会话是哪一个。
- 每个角色之前对应哪个长期会话。

第一阶段先落地项目创建和项目经理主导；当前实现已把角色委托绑定写入数据库，并在
后续委托时把 `conversation_id` / agent 原生 `external_id` 传回启动链路，用于跨重启
恢复同一个长期 collaborator。后续主要补项目控制台与可视化管理。

## 建议分阶段实现

### 第一阶段：MVP

实现最小闭环：

- 新建协作项目。
- 选择工作目录。
- 配置角色。
- 强制包含项目经理。
- 创建项目经理会话。
- 打开项目经理 tab。
- 不向会话输入框注入项目提示词。
- 角色信息保存到 `project_role`，后续委托链路按 `role_key` 读取。

该阶段的重点是把项目创建流程和项目元数据持久化跑通，并让项目经理成为用户和多智能体协作之间的可恢复入口。

### 第二阶段：角色长期会话绑定

增强持久化能力：

- 每个角色绑定 `conversation_id`。（已实现写回）
- 项目经理委托时自动复用项目角色会话。（已实现工具层强制 `reuse = true`）
- 通过 `list_project_roles` 向项目经理会话暴露机器可读角色清单。（已实现）
- 重启后恢复项目角色关系。（已实现：DB 读取 `conversation_id` + `external_id`，
  broker spawn 时续接 agent-native session，发送任务时复用既有 Billow conversation）
- 项目详情页显示每个角色状态和对应 tab。

### 第三阶段：项目控制台

扩展项目管理视图：

- 项目列表。
- 项目详情页。
- 角色卡片。
- 一键打开项目经理。
- 一键打开某个角色会话。
- 并列查看所有角色标签页。
- 项目级任务进度和角色最近活动。

## MVP 数据流

```text
用户点击“新建协作项目”
  -> 选择工作目录
  -> 配置角色
  -> 创建 / 打开 folder
  -> 写入 project / project_role
  -> 创建项目经理 conversation
  -> 打开项目经理 tab
  -> 用户向项目经理描述需求
  -> Billow 根据项目角色触发/约束项目委托
  -> 角色长期 collaborator 执行任务
  -> 项目经理收集结果并汇总
```

## 需要注意的问题

- 项目经理必须唯一且不可删除。
- 角色 `role_key` 应稳定、唯一、适合作为 MCP `role`。
- 角色名称可以展示给用户，`role_key` 用于系统关联。
- 工作目录应复用现有 folder 机制，不重复造目录模型。
- 不要把项目功能和现有脚手架 Project Boot 混成同一个概念；可以共享入口或 UI 组件，但语义上应区分“代码脚手架创建”和“协作项目创建”。
- 跨重启长期协作不能只依赖 broker 内存缓存；必须以 `project_role.conversation_id`
  和 `conversation.external_id` 作为 durable binding。
