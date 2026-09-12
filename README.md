# Grok-Bot-Auth

Standalone Rust desktop app that lets [Cursor](https://cursor.com) use a **Grok Bot sand session** (and imported providers) next to an official Cursor subscription.

- Official unsuffixed models (for example `grok-4.6`) stay on `api2.cursor.sh`.
- Enabled models are injected as `gb-*` (display suffix `· Grok Bot` / `· xAI` / provider label) via `127.0.0.1:47821`.
- Coexist is a Cursor extension **hook**, not MITM. No `http.proxy`, no root CA.
- When this app is not running, the hook fail-opens to official Cursor.

License: [MIT](LICENSE).

## Install

Windows release folder (no zip): `Grok-Bot-Auth-<version>/Grok-Bot-Auth.exe` next to this repo, or build from source:

```bash
cargo run --release
```

Data dir: `~/.grok-bot-auth/` (tokens, never commit).

## Cursor coexist

1. Sign in / import a Grok Bot session in this app.
2. Sync models, enable the ones you want, then **Enable coexist**.
3. **Fully quit Cursor and reopen** (Reload is not enough).
4. Official models stay official. Use `gb-*` / `· Grok Bot` / `· xAI` for this app.
5. Close-to-tray keeps 47821 up. Tray **Quit** restores Cursor files.

Do not run cursor-byok MITM at the same time.

OpenAI-compatible local API: `http://127.0.0.1:47821/v1`

## Constants (not secrets)

| Item | Value | Why |
|------|--------|-----|
| Bind | `127.0.0.1:47821` | Local coexist entry |
| Cursor sand API | `https://api2.cursor.sh` | Official sand backend |
| Cursor OAuth client id | public desktop client id | Same as Cursor login |
| Fallback Cursor version stamp | installed Cursor `package.json`, else `3.20.17` | api2 rejects outdated `0.47.0` |

## Dev

```bash
cargo test
```

`probe-chat` is an optional diagnostic binary, not the desktop app.
