# Grok-Bot-Auth

## 0.4.21 — 2026-09-13

- 选择器注入改为 Rust/SQLite UTF-8，不再弹 Python 黑窗，也不再把 UTF-8 读成系统代码页（`· 通用` 乱码）。
- 反代模型单独一节（vendor = Grok Bot / xAI / 通用），variant 显示名带后缀，不再长得像官方 grok-4.6 High Fast。
- 停用共存会清掉 gb-* 和乱码残留。

## 0.4.20 — 2026-09-13

- 共存目录合并改在官方 HTTP/2 流上原地追加 `gb-*`。
- 同时写入 Cursor Settings 的 `availableDefaultModels2` 缓存并打开开关，否则官方列表回来后选择器仍看不到 · Grok Bot。
- 修复窗口/托盘图标（恢复后的 icon.rgba 曾全是 0）。

## 0.4.19 — 2026-09-13

- 开共存不再写全局 `cursor.general.disableHttp2`（那会波及 Plan & Usage）。Agent 仍只改两个工厂为 HTTP/1.1。
- 停用共存 / 退出 / 未开共存启动时，清掉 bak 里残留的 Agent HTTP/1 和 WebSocket 闸。Plan / GetMe / 用量 RPC 从不进 47821。

## 0.4.18 — 2026-09-13

- 共存时官方模型列表不再被 47821 整表替换。AvailableModels / GetUsableModels 先走官方 api2，本机只把 `gb-*` 追加进去。HTTP/2 同样不再 divertH1 掉官方目录。
- 停用原先那份过小的 sand 目录缓存当官方列表。

## 0.4.17 — 2026-09-13

- 官方共存隔离：hook + `127.0.0.1:47821` fail-open。无前缀官方模型（如 `grok-4.6`）直连 api2；只有启用过的 `gb-*` 在本软件运行时进本机。不写 `http.proxy`、不装根证。MITM 47822 停用。
- 关掉本软件（托盘「退出」）会撤 hook / WebSocket 闸；47821 不通时 Cursor 走官方。关窗口只进托盘，进程还在。
- 托盘「退出」不再被进托盘后粘住的 `close_requested` 吃掉；点退出会 park 后结束进程。
- 官方 Agent 不再被 30 秒 `injectUntil` 偷走；HalfClose ≠ Cancel；CORS 仅本机 Origin；Cursor 升级会重快照 bak。
- 压缩保前导 system 与最新 tool_result；已删供应商的 `gb-p/...` 不再落到 Grok Bot sand。
- MCP 写入 `mcp.json` 需确认，stdio command 白名单。Blob 盘名按 id 的 sha256 hex，Upload 走 CAS。

## 0.4.16 — 2026-09-12

- MITM RunSSE 与 HTTP 入口一样：先等 BidiAppend 配对再决定本地/官方转发，官方不再空等 api2。
- Shell 执行中按 CCursor 发 `toolCallDelta` stdout/stderr，终端不再等整段结束才出字。
- 原生 tool_calls 参数不完整时回退 XML；Grok Stream / xAI 按 system/user/tool_result 分轮发送。

## 0.4.15 — 2026-09-12

- 对照 CCursor：注入 Agent 发 `conversation_checkpoint_update`（token_details used/max=256K），右下角上下文条不再只靠 tokenDelta。
- 解码 `conversation_history` / prepend，多轮不再只剩当前一句。
- `partialToolCall` + `stepStarted/Completed` 对齐官方帧序。
- 不再用正文里的 `gb-` 扫描劫持官方 Bidi；exec `id=0` 不再误配工具结果。
- xAI ChatCompletions 带上 `reasoning_effort`；供应商流超时 300s。官方 unsuffixed 仍走 api2。

## 0.4.14 — 2026-09-12

- Grok Bot Stream 不再把工作区文件/目录树塞进 prompt；用户长上下文保留末尾（真正的问题），超限时自动再截一档。
- 思考和 token_delta 边生成边发给 Cursor，不再等整轮 LLM 结束才出字（xAI 延迟、右下角上下文条、Bot 无思考）。
- TurnEnded 带 input/output token；GetUsableModels JSON 带 256K contextTokenLimit。
- 官方 unsuffixed 模型仍走 Cursor api2 / 官方账号。思考流和截断只作用在 `gb-*`（Grok Bot / 已启用的供应商）。

## 0.4.13 — 2026-09-12

- 重置时间按 RFC3339 / protobuf seconds 解析，不再把日期数字当毫秒（避免「2742 天后重置」）。
- 账号页去掉「添加 Grok Bot 账号」和手动刷新；Grok Bot 登录只在供应商页；额度每分钟自动拉 Grok Bot 周用量和 xAI credits。
- 额度共享/优先按供应商分开；xAI OAuth 出现在第三方列表，不进 Grok Bot 池、不显示 Cursor 套餐。

## 0.4.12 — 2026-09-12

- Grok Bot 额度改为 `GetSandUsageStatus`（周用量百分比，对应 Spending 里 Grok Bot 33%）。不再请求、不再显示 Cursor Models / Other Models。

## 0.4.11 — 2026-09-12

- Grok Bot「额度用尽」误用 Cursor Models / Other Models 的 GetCurrentPeriodUsage。只用 Grok Bot 自己的用量桶；没有该桶时不把套餐 99%/100% 标成 Grok 用尽。
- xAI OAuth 写入供应商账号列表（无 email 时用 JWT sub），并留在供应商页，不再跳到 Grok Bot 账号管理。第三方 OAuth 不进 Cursor 额度池。

## 0.4.10 — 2026-09-12

- 已登录但仍提示「没有可用 Grok Bot 账号」：账号池被标成额度用尽后 `pick` 直接空。重新登录会清标记；若全部用尽则回退当前令牌。浏览器 All set / Open Grok Bot 是官方应用，不会把令牌写进本软件。

## 0.4.9 — 2026-09-12

- xAI 重新授权后 refresh 被「再点保存」和 prefs/providers 双份存储冲掉，6 小时后无法自动续。保存时不再清空已有 refresh；启动时合并两份；授权成功会同时写入 prefs。

## 0.4.8 — 2026-09-11

- 审查修复：官方 RunSSE 先连上 api2 再放行 Bidi；不再用全文扫描 `gb-` 劫持官方请求；转发时去掉已解压的 gzip 头。
- exec 乱序不再丢掉；xAI 多 tool_calls 都会进 host-exec；Shell `close_stdin`；附件 UTF-8 截断不再 panic。

## 0.4.7 — 2026-09-11

- 官方模型共存：HTTP/1.1 先 RunSSE 后 BidiAppend。本地不再把 RunSSE 卡 2 秒再整包转 api2（那会让官方对不上、30 秒超时）。官方 RunSSE 立刻流式转发；Bidi 等 RunSSE 先接到 api2。
- Grok Bot Stream 现在带 Read/Grep/Glob/Ls/Shell/Edit/Write；`tool_call_part` 不再丢掉。
- xAI 思考：解析 `reasoning_content`，思考帧带 `thinking_style=DEFAULT`。

## 0.4.6 — 2026-09-11

- 注入 `gb-*` 走 Cursor 官方 Agent 工具环：`tool_call_started` / `exec_server_message` / 等 BidiAppend `exec_client_message` / `tool_call_completed`，由 Cursor 本机执行 Read、Grep、Glob、Ls、Shell、Edit、Write。不再在 GBA 里读盘。RunSSE / AgentService/Run / WS 保持流开着直到工具回来。
- `encode_local_completion` 不再是注入 Agent 的收尾。
- xAI / OpenAI 兼容供应商请求带上 Cursor 同名 tools（Read/Grep/Glob/Ls/Shell/Edit/Write），流式 `tool_calls` 转成 host-exec 帧。本地 Bidi 的 streamClose 不再误转到 api2。

## 0.4.5 — 2026-09-11

- 注入模型不再只回聊天：解析 Cursor `RequestContext`（工作区路径、附带文件），把目录树 / git / README 喂给模型，并支持 `read_file` / `ls` / `grep` 本地工具轮。思考结束发 `thinking_completed`。
- 选择器双重 `· xAI` / `· 通用`：后缀只加一次，变体名不再二次替换。

## 0.4.4 — 2026-09-11

- Cursor 里 Grok Bot 能回、xAI 显示 Unexpected error：xAI OAuth 访问令牌已过期，错误被 Cursor 吃成笼统失败。过期时提示重新授权；有 refresh 会自动换票。供应商行显示 `· xAI`，不再长得像官方 High Fast。

## 0.4.3 — 2026-09-11

- Cursor 选 `· Grok Bot` 仍 BAD_MODEL_NAME：HTTP/1.1 Agent 把 Run 拆成 RunSSE（只有 request_id）先发、BidiAppend（模型）后到。空 RunSSE 被转给官方，官方再看到 `gb-*` 就拒。RunSSE 最多等 2 秒等 BidiAppend；HTTP/Connect gzip 先解开再扫 `gb-*`。

## 0.4.2 — 2026-09-11

- Cursor 选注入模型发 `hi` 仍报 “The model you chose is not available”：目录已经注入 `gb-grok-4.6`，发送走 `agent.v1.AgentService/Run` 打到官方 `api2.cursor.sh`，官方回 `ERROR_BAD_MODEL_NAME` / `Unknown model ID: gb-grok-4.6`。钩子 HIT 以前只有 `RunSSE` / `BidiAppend`，HTTP/1.1 也接不住 `…/Run`。
- 钩子增加 `agent.v1.AgentService/Run`；47821 对 `gb-*` 走 Grok Bot Stream，官方 id 仍转发 api2。HTTP/2 会话再包一层 `http2.connect`（always-local 里 Agent 传输仍可能写死 h2）。
- 输入框短名带 `· Grok Bot` / `· xAI`，避免看起来像官方 High Fast。
- 09:16 实测：诊断 HTTP/1.1 仍建了 Agent HTTP/2，`Run` 直连 api2；钩子用包装后的 `http.request` 回环会再次 HIT。改为未包装直连 47821，并把 Agent/AgenticComposer 工厂写成 `useHttp2:false`。
- 关窗口进托盘后「打开」没反应：`close_requested` 一直为真，每帧又把窗口藏起来。只藏一次；托盘线程用 Win32 `ShowWindow` 把窗口拉回来。
- 概览时间芯片的 Overview 图标被看成「0o」；贡献日历按剩余列宽/200px 高放大，不再是 11px 小格。

## 0.4.1 — 2026-09-11

- 开着 Grok-Bot-Auth Cursor 反而不行、关掉才正常：反代把 `GetDefaultModel` 改成 `gb-*`，并用过期 Statsig 缓存顶掉官方账号闸。对照 CCursor：Statsig 直通官方；默认模型不再劫持。
- 关窗口进托盘（和 cursor-byok 一样），MITM 继续跑。只有托盘「退出」才撤 Cursor 代理。

## 0.4.0 — 2026-09-11

- **直连 ≠ 关掉反代。** Clash / 系统出站代理可以没有；Cursor 反代（MITM 47822）仍要开着才能注入 `gb-*`。
- 账户栏/用量空：旧版本把 `http.proxyKerberosServicePrincipal` 写成了 47822（那不是 Kerberos SPN）。已停止写入并清掉。
- Cursor 转圈：`/agent/v1/run` 的模型在 **WebSocket 帧**里（protobuf 里能看到 `gb-`），HTTP Bidi 经常只是心跳。反代现在读 WS 帧并本地补全，官方模型仍转给 api2。
- 关掉本软件时暂时撤掉 Cursor 里的 47822，避免空等死端口；`共存` 偏好保留，下次打开自动再接反代。

## 0.3.9 — 2026-09-11

- Cursor 转圈：BidiAppend 里模型在 hex protobuf 中，旧 ASCII/`gb-grok-4.6` 精确匹配经常 miss，请求被原样交给官方。改为扫描 `gb-` 并本地 RunSSE。
- 测试对话：默认选已启用的可用模型（Grok 4.6），不再按目录字母落到 Haiku；点卡片不会跳到模型调度。气泡改顶部滚动，避免挡住输入框。

## 0.3.8 — 2026-09-11

- 测试对话 `ERROR_NOT_LOGGED_IN`：过期的 cursorvm `box-stream.json` HTTP 200 带 Connect 错误，排在 api2 前面，真正的 Stream 根本没打。改为先 `api2.cursor.sh`。
- Cursor 卡住：Grok-Bot-Auth 没在跑时 `http.proxy=127.0.0.1:47822` 会空等。必须让本软件一直开着。

## 0.3.7 — 2026-09-11

- 发送仍失败：Statsig 闸关掉之后 Cursor **照样**升级 `wss://…/agent/v1/run`。和 CCursor 一样，共存时改本地 `cursor-always-local` / `cursor-agent-host` 里的 `nal_websocket_client` 闸名，聊天才能回到已接好的 Bidi+RunSSE。
- 打开卡 10 秒：AvailableModels / Statsig 有缓存就立刻返回，后台再刷新。禁止 4 秒超时后塞空目录（那会把官方模型元数据弄丢）。
- 官方 grok-4.6 上下文本来就是 256K/256K，没有 200K/500K 旋钮；注入模型不再改官方那条。

## 0.3.6 — 2026-09-11

- 发送失败根因：`BootstrapStatsig` 响应经常是 gzip，补丁解不开，Cursor 继续走官方 WebSocket，`gb-*` 仍是 BAD_MODEL_NAME。在 MITM 响应里解压再关 `nal_websocket_client`，不再把 700KB Statsig 绕一圈本地端口。
- Cursor 打开卡约 10 秒：启动时 AvailableModels / UsableModels 上游最多等 4 秒，超时用上次缓存；代理日志不再每条 RPC 写盘。
- 供应商模型注入对照 Grok Bot / Cursor 官方目录：有 Fast 才给 Fast，上下文按官方窗口固定（grok-4.6 = 256K，不编 500K），思考档按官方 `effort`/`reasoning`（grok-4.6 = Low/Medium/High/Extra High，不编 Max）。目录没有的模型不编旋钮。
- CCursor：`parameters.effort[]` + `parameters.fast` + 固定 `contextTokenLimit`；cursor-byok 会对所有 BYOK 填 context/reasoning/fast，这里不跟那套最高能力表。

## 0.3.5 — 2026-09-11

- 模型调度：同名模型按供应商区分（`grok-4.6 · X` 与 `Cursor Grok 4.6 · Grok Bot`），开关不再连坐。
- 导入官方清单不再填 Low…Max / 200k；第三方映射只保留上游 id。
- Grok Bot 登录卡片去掉「退出当前」「账号管理」（账号页已有入口）。
- 网络服务增加可选全域出站代理（Clash 等 `http://127.0.0.1:7890`）。

## 0.3.4 — 2026-09-11

- 查清 “The model you chose is not available”：Cursor 3.20 把聊天发到 `wss://api2.cursor.sh/agent/v1/run`。HTTP 升级 body 是空的，模型名在 WebSocket 帧里，反代看不见 `gb-*`。官方 WS 回 `ERROR_BAD_MODEL_NAME`。
- 503 掉 WebSocket **不能**逼回 HTTP：`fallbackOnColdPool` 默认是 false。CCursor 的做法是关掉 Statsig 闸 `nal_websocket_client`。共存时改写 `BootstrapStatsig`，把该闸设为 false，聊天回到 BidiAppend + RunSSE，本地才能吃 `gb-*`。
- 必须**完全退出** Cursor。已建立的 WS 池和闸缓存活到进程结束；Reload 不够。
- Cursor 选择器不加「可用」徽章。Grok 4.6 Stream 窗口按官方目录是 **256K / 256K**，不是 500K。

## 0.3.3 — 2026-09-11

- Cursor 选择器不再打「可用 / GROK BOT」徽章。
- Grok 4.6：Grok Bot 目录是 256K；Cursor 官方 200K/500K 是 Max Mode 窗口，不要当成 Stream 已经有 500K。

## 0.3.2 — 2026-09-11

- 独立「账号」页：按邮箱管理多个 Grok Bot / Cursor 会话。
- 调配方式：额度共享（轮询）或额度优先（用尽再切下一个）。
- OAuth 显示登录网址，可复制、中途取消，或点「已完成，核实」。
- 额度展示对齐 [Cockpit Tools](https://github.com/jlcodes99/cockpit-tools) 的 Cursor 口径：`GetCurrentPeriodUsage` 剩余百分比 + 重置时间进度条。
- 测试对话在额度不足时自动切到池里下一个未耗尽账号。

## 0.3.1 — 2026-09-11

- Cursor 注入不再硬编 Low…Max / 200k / 500k。旋钮从官方 AvailableModels 的 parameterDefinitions 和 variants 克隆；找不到官方条目时才按该模型自己的 axes 生成。
- Grok 4.6 官方轴是 `effort=low|medium|high|xhigh` + `fast`，默认 `grok-4.6[effort=high,fast=true]`。没有 `reasoning`，也没有捏出来的 `context=200k`。这是 Extra High 报 “not available” 的根因。
- 供应商 → Grok Bot：登录卡片上「同步模型」；用户勾选添加，本页显示是否可用。不再把整份目录灌进配置。
- 模型调度去掉「启动当前模型」和「同步模型」。只列出已添加的模型，右侧开关才注入 Cursor。
- 第三方映射不再默认填 Low…Max / 256K。

## 0.3.0 — 2026-09-11

- Cursor 只接入软件里**已启动**的模型，不再把整份目录灌进 Settings / 选择器。
- 注入模型的上下文按官方窗口固定（不再提供 200k/500k/1m 三选一和软件打架）。
- 思考强度：Cursor 只能在软件允许的 Low…Max 里选，请求按 Cursor 当前档位发给 Stream。
- 版本页：固定 0.3.0、更新日志、检查更新；探测 Cursor 不再弹命令窗口。
- 概览：时间轴柱状趋势（无调用也画灰柱）、成功率仪表、贡献日历、最近请求表。
- 同步目录不再把全部模型标成已启动。

## 0.2.0 — 2026-09-10

- 供应商 BYOK、Grok Bot OAuth、模型映射、提示池、测试对话。
- 与 Cursor 共存（gb- 前缀注入）。
