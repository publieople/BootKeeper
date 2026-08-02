# BootKeeper

Windows 自启动管理工具：现代 UI（Material Design 3）、中英 i18n、AI 可调用。

开源免费（GPL-3.0）。当前处于早期开发：**M1 完成（CLI 读链路）**。

## 状态

| 里程碑 | 内容 | 状态 |
|---|---|---|
| M0 | workspace + core（枚举 + 规则引擎 + 快照） | ✅ |
| M1 | CLI 读链路 + SKILL.md | ✅ |
| M2 | helper（提权 + 确认窗 + 写操作） | ⏳ |
| M3 | GUI（Naive UI + MD3 + i18n） | 未开始 |
| M4 | AI 全链路 + 打包 + MCP 适配层 | 未开始 |

## 构建与测试

```sh
cargo build
cargo test --workspace
```

CI（GitHub Actions）：Linux 跑纯逻辑测试，Windows runner 跑真实注册表/任务计划/签名校验。

## CLI（v1 读链路）

```sh
bootkeeper list [--category registry_run|startup_folder|scheduled_task]
bootkeeper get <id>
bootkeeper analyze [--category ...]
bootkeeper snapshot list | show <id>
```

所有输出为 JSON，供 AI agent 直接解析。写操作（M2）需提权 + 用户确认弹窗。

## AI 集成

标准 SKILL.md（skills.sh 规范）：`npx skills add publieople/BootKeeper`

## 文档

- [PLAN.md](PLAN.md) — 完整项目计划与决策记录
- [SKILL.md](SKILL.md) — AI agent 使用指南
