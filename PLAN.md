# BootKeeper — 项目计划

Windows 自启动管理工具：开源、现代 UI（Material Design 3）、中文优先的 i18n、AI 可调用（MCP）。

> 本文档由 grilling 会话打磨成型。所有安全边界、架构决策均经过拷问确认。

## 定位

- **差异化**：现代 UI + i18n + AI 交互，不是"管理启动项"本身。
- **对标缺口**：Sysinternals Autoruns 界面过时且无中文；CCleaner 收费；其他开源 Startup Manager 无 AI 交互。
- **形态**：开源免费（MIT），Windows 桌面应用。

## v1 范围（三件套）

| 启动方式 | 说明 |
|---|---|
| 注册表 Run / RunOnce | HKCU + HKLM |
| 启动文件夹 | 用户级 + 系统级 |
| 任务计划 | Task Scheduler |

服务 / WMI / 组策略 / IFEO 等后期扩展。枚举位置类别参考 `p0w3rsh3ll/AutoRuns`。

## 技术栈

```
Tauri 2 (Rust) + Vue 3 + TypeScript + Naive UI
+ @material/material-color-utilities (MD3 动态取色，参考 motrix-next 的做法)
+ Pinia + vue-i18n + tauri-plugin-locale-api + Vite
```

参考项目：[motrix-next](https://github.com/AnInsomniacy/motrix-next)（Tauri 2 + Vue 3 + Naive UI + MD3 取色 + 52 语言 i18n + sidecar 模式）。

## 架构：三进程

| 进程 | 权限 | 职责 |
|---|---|---|
| **sidecar** (`app --daemon`) | 普通，常驻 | 开机自启（Run 键，dogfood）；MCP server（HTTP localhost + token）；读操作直答；写操作转发 helper |
| **helper** (`app --exec`) | 提权，一次性 | `ShellExecute runas` 拉起；弹独立确认窗（不依赖 GUI）；执行写操作后退出 |
| **GUI** | 提权可重启 | 管理界面；UAC 重启自己；与 sidecar IPC 通信 |

## 安全模型（核心）

1. **硬确认 token**：写工具（delete/enable/disable/add）默认拒绝执行，必须先拿到确认 token（确认窗通过后颁发，带超时）。AI 不弹窗就什么都改不了。边界在运行时，不在 AI 自觉。
2. **弹窗事实独立核验**：helper 自己查注册表、验签名（WinVerifyTrust Authenticode）、跑规则引擎——**不信 sidecar 传来的文本**。sidecar 只传"操作意图 + 条目 ID"。
3. **风险定级 = 软件规则**：未签名 + 非系统目录 + 非微软发布者 = 高风险。AI 只做分析建议，用户最终拍板。
4. **规则引擎一份代码**：sidecar / helper / GUI 共享核心 crate（Rust workspace），禁止各写一遍。

## 快照与恢复

- 每次写操作前自动备份，记录操作前后快照。
- 快照默认保留 7 天（可配置）。
- 禁用语义 = **改名**（`Foo` → `Foo.disabled`，Autoruns 官方思路）；恢复 = 改回名字。
- MCP tool surface 含 `restore_item`。

## MCP tool surface（草案）

| 工具 | 读/写 | 说明 |
|---|---|---|
| `list_items` | 读 | 枚举启动项，按类别过滤 |
| `get_item` | 读 | 单条详情（路径/签名/发布者/风险等级） |
| `analyze_items` | 读 | AI 辅助分析（只建议，不定级） |
| `enable_item` / `disable_item` | 写* | 改回/追加 `.disabled` 后缀 |
| `remove_item` | 写* | 删除条目（先备份） |
| `add_item` | 写* | 新增条目 |
| `restore_item` | 写* | 从快照恢复 |
| `list_snapshots` | 读 | 查看快照历史 |

*写工具必须持有有效确认 token 才执行。

## 待定问题（写代码前逐一定）

- [ ] Rust workspace 划分（core / mcp-server / helper / gui）
- [ ] MCP server 实现 crate（rmcp 或 tauri 侧自实现）
- [ ] token 存储（Windows Credential Manager / DPAPI vs 配置文件）
- [ ] i18n 规模（52 语言 or v1 只做 zh/en）
- [ ] 签名校验信任锚（微软证书列表怎么维护）
- [ ] 任务计划实现（COM API vs schtasks CLI 包装）

## 里程碑

- **M0**：workspace 骨架 + core（枚举三件套 + 规则引擎 + 快照）
- **M1**：sidecar（MCP server + token + HTTP）跑通读链路
- **M2**：helper（提权 + 独立确认窗 + 写操作 + 硬确认 token）
- **M3**：GUI（Naive UI + MD3 + i18n 骨架）
- **M4**：AI 全链路联调 + 快照恢复 + 打包发布

## 参考

- 功能清单：https://github.com/p0w3rsh3ll/AutoRuns
- 技术参考：https://github.com/AnInsomniacy/motrix-next
- 内置 MCP 应用先例：https://github.com/Env-Kit/envkit-releases
