# Grok-Bot-Auth 使用说明

当前版本 **0.4.21**。下载：[Releases](https://github.com/HarryPD168/grok-bot-auth/releases/tag/v0.4.21)

## 第一次用

1. 关掉正在运行的旧 `Grok-Bot-Auth`（点托盘 **退出**，不要只关窗口）。
2. 运行 `Grok-Bot-Auth.exe`。
3. 登录或导入 Grok Bot 会话（已登录会显示绿色）。
4. 打开 **模型**：点获取 / 同步，勾选要用的，打开右侧开关。
5. 供应商模型在 **供应商** 里导入 Key 或 OAuth，同样打开开关。
6. 点 **启用共存**。
7. **完全退出 Cursor 再打开**（不要只点 Reload / Restart）。
8. 在 Cursor 里：官方无前缀模型照常用；要用本软件请选带 **`· Grok Bot` / `· xAI` / 供应商标签** 的项。

登录态和密钥在 `C:\Users\<你>\.grok-bot-auth\`，不要把这个目录发到网上。

## 和官方订阅怎么分工

| | 官方 Cursor | 本软件 |
|--|--|--|
| 列表里 | `Cursor Grok 4.6`、Claude、GPT 等，无后缀 | `Cursor Grok 4.6 · Grok Bot`、`· xAI`、`· 通用` |
| 流量 | 直连 api2，走 Cursor 额度 | `127.0.0.1:47821`，走 Grok Bot / 你的 Key |
| 本软件没开 | 正常用官方 | `gb-*` 不可用，官方不受影响 |

选择器里官方和反代是分开的。反代项按供应商标签区分。

## 关窗口、停用、退出

| 你点的 | 实际 |
|--------|------|
| 窗口关闭 | 进托盘，本软件还在跑，共存还在 |
| **停用共存** | 卸 hook；必须再完全退出 Cursor |
| 托盘 **退出** | 停进程并恢复 Cursor 文件；完全退出 Cursor 后官方即原厂 |
| 任务管理器结束 | 可能来不及恢复 hook，下次开本软件或手动停用共存再重启 Cursor |

## 常见问题

**列表里没有 · Grok Bot**  
确认本软件在跑、模型开关已开、已启用共存，然后 **完全退出 Cursor 再开**。在 Settings → Models 搜 `Grok Bot` 或 `gb-`。

**官方模型变少 / 套餐转圈**  
先托盘退出本软件，完全退出 Cursor 再开。不要写 `http.proxy`。不要和 cursor-byok MITM 一起用。

**启用/停用共存弹出黑窗**  
0.4.21 起不应再弹。请用本页开头的 Release，不要用旧 exe。

**乱码（路、閭氟叴）**  
用 0.4.21，再点一次启用共存，然后完全退出 Cursor。

**图标是黑块**  
旧包图标文件损坏。用 0.4.21 的 exe。

**关了窗口官方还走本软件**  
关窗口只是托盘。要停请托盘退出或停用共存。

## 给其它软件的 API

`http://127.0.0.1:47821/v1`（OpenAI 兼容）。本软件必须在跑。

## 从源码编译

```bash
cd grok-bot-auth
cargo run --release
```
