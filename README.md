# Grok-Bot-Auth

独立桌面程序（Rust）。在 **Cursor 官方订阅账号继续登录** 的前提下，把 **Grok Bot sand 会话** 和导入的供应商模型接到 Cursor 里用。

许可证：[MIT](LICENSE)

## 它做什么

| | 官方 Cursor | 本软件启用的模型 |
|--|--|--|
| 模型 id | 无前缀，例如 `grok-4.6` | `gb-*`，展示名带 `· Grok Bot` / `· xAI` / 供应商标签 |
| 流量 | 直连 `api2.cursor.sh` | 本机 `127.0.0.1:47821` |
| 额度 | Cursor 订阅 | Grok Bot / 你导入的 Key |

共存靠 Cursor 用户级 **hook**，**不是** MITM：不写 `http.proxy`、不装根证书。本软件没在跑时，hook 会 fail-open，官方聊天仍走 api2。

## 安装（Windows）

1. 打开 [Releases](https://github.com/HarryPD168/grok-bot-auth/releases)，下载当前版本目录里的 `Grok-Bot-Auth.exe`。
2. 关掉正在运行的旧进程（托盘「退出」，不要只关窗口）。
3. 运行新 exe。数据在 `~/.grok-bot-auth/`（登录态、密钥，不要提交到 git）。

从源码编译：

```bash
cargo run --release
```

## 和官方 Cursor 共存

1. 在本软件登录或导入 Grok Bot 会话。
2. **获取模型** → 勾选要用的 → 打开开关启用。
3. 点 **启用共存**。
4. **完全退出 Cursor 再开**（Reload 不够，必须结束进程）。
5. 官方无前缀模型照常用；要用本软件请切带 `· Grok Bot` / `· xAI` / 供应商标签的项。

选择器里官方订阅和反代模型是分开的，反代项按供应商标签区分。

关窗口只会进托盘，`47821` 还在听。要停共存：点 **停用共存**，或托盘 **退出**，然后再完全退出 Cursor。

不要和 cursor-byok 的 MITM 同时打 Cursor 补丁。不要再配 `http.proxy` 到 `47822`。

给其它客户端的 OpenAI 兼容口：`http://127.0.0.1:47821/v1`

## 开发

```bash
cargo test
```

`probe-chat` 是可选诊断工具，不是桌面主程序。

## 常量（不是密钥）

| 项 | 值 |
|----|-----|
| 本机入口 | `127.0.0.1:47821` |
| Cursor sand | `https://api2.cursor.sh` |
| Cursor / xAI OAuth client id | 公开桌面客户端 id |
| Cursor 版本戳 | 读取已安装 Cursor 的 `package.json`，否则 `3.20.17` |
