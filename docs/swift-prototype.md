# llavero

Local key vault for coding agents. The agent creates a slot by name, you paste the
value in a native macOS window, and the agent only ever gets `llavero://NAME`.

- Values: macOS login Keychain (`Sources/LlaveroCore/KeychainStore.swift`), encrypted at rest.
- Names and project assignments: `~/Library/Application Support/Llavero/registry.json` (no values).
- `run` injects only the project's keys and masks their values in the child's output.

```sh
swift build -c release
.build/release/llavero create EXA_API_KEY --project my-app   # opens the paste window
.build/release/llavero list
.build/release/llavero app                                   # window: names, status, projects (never values)
.build/release/llavero run --project my-app -- npm run dev
swift test
```

## MCP server

`llavero mcp` speaks MCP over stdio. Tools: `list_secrets`, `create_secret` (opens the
paste window), `request_secret_value`, `assign_secret`. None returns a value.

```sh
claude mcp add --scope user llavero -- "$PWD/.build/release/llavero" mcp
```

Limits: a child started by `run` holds the real value and could write it somewhere
other than stdout/stderr (a file, the network, encoded). Masking covers the plain value only.
