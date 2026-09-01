# StoryWalk — AI 小说生成器（Tauri 桌面应用）

> 通用项目信息（简介、功能特性、开发命令、技术栈、项目结构）见 [README.md](README.md)。本文件仅记录 AI 编码代理需要遵守的工作约定。

## 前后端通信

**不是 REST API。** 前端通过 `@tauri-apps/api/core` 的 `invoke()` 调用 Rust 端 `#[tauri::command]`。流式输出通过 Tauri Event 系统推送。

所有函数参数字段名使用 camelCase（Rust 端自动转换 snake_case）。

## 聊天模式与素材系统

- **单一写作会话（creation）**: AI 读取素材生成故事正文，剧情通过 `save_story_card` 工具沉淀为左侧剧情卡片。消息构建顺序：system（含 reference.md 相关资料）→ 历史消息 → guidelines.md 创作准则 + 用户消息。会话内另有卡片工具 `read_story_cards` / `read_story_card` / `update_story_card`，支持 `[卡片:第N轮](card-xxx)` 引用标记。
- **后台素材沉淀（extraction）**: 写作回复完成后（本轮产出过剧情卡片时），后台隐藏会话（mode='extraction'）自动执行素材提取——重放写作会话历史，用素材工具将新设定/文风偏好简洁沉淀到素材文件，前端展示「素材沉淀中」状态。素材工具：`read_story_md`（读取素材）、`patch_story_md`（增量编辑，old_str 必须在文件中精确唯一匹配）、`update_story_md`（全量重写，仅文件为空或大规模重构时使用）。
- 素材文件位于 `stories/<storyId>/` 目录：
  - `reference.md`（相关资料）注入 System Prompt，作为全局上下文，适合角色设定、世界观、故事背景等全篇参考的信息
  - `guidelines.md`（创作准则）注入最新用户消息末尾（近因位置），约束力最强，适合写作风格、叙事规则、禁止句式等每轮严格执行的规范
  - 创建故事时选择文风（现代 / 古代），对应文风种子（含补充规则、自省约束、示例对白库）自动写入该故事 guidelines.md 作为初始创作准则

## 重要注意事项

- **本项目是 Tauri 桌面应用，不是 Next.js 项目，也不是纯 Web 应用。** Vite + React 做前端 UI，Rust 做原生后端。
- 开发运行：`npm run tauri dev`（自动起 Vite + 编译 Rust + 打开原生窗口）。
- 生产发布：`tauri build`。
- 环境变量 `DEEPSEEK_API_KEY` 配置在 `.env` 文件中。
- `src-tauri/capabilities/default.json` 控制 Tauri 权限。
- 数据文件 `data.db` 位于项目根目录（开发模式），**删除必须获得用户明确批准**。
- `reference/BitFun` 是外部参考项目 submodule，**仅可阅读参考，禁止编辑、修改或提交其任何内容**。
- 开发原则：优先参考成熟开源项目的实现，理解其设计模式和实现方式再落地，避免凭空设计。
- 不要擅自替用户做决策，遇到需要决策的内容，需要抛给用户确认。
