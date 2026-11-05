<div align="center">

<img src="docs/images/icon.png" width="96" height="96" alt="Keyfold icon">

# Keyfold

**A key wallet for your coding agent: it can use your API keys, but never see them.**

Keep the keys of all your projects in one place. Paste each key once in a native window; Keyfold stores it<br>
encrypted in your OS credential store, hands your agent a reference, and injects the real value only when your app runs.

[How it works](#how-it-works) · [Install](#install) · [Connect your agent](#connect-your-agent) · [Commands](#commands) · [Security](#security)

![macOS](https://img.shields.io/badge/macOS-tested-d6336c)
![Windows](https://img.shields.io/badge/Windows-builds%20in%20CI-6a6a76)
![MCP](https://img.shields.io/badge/MCP-stdio-d6336c)
![Local only](https://img.shields.io/badge/cloud-none-6a6a76)

</div>

<br>

<p align="center">
  <img src="docs/images/keys-window.png" width="820" alt="Keyfold keys window: names, status and projects of each key, never the value">
</p>

## Why it exists

Keyfold is a personal project: the tool I use every day to work with my own coding agents.
The keys of all my projects live in it, and my agents run those projects with them without ever reading one.
It is open source in case it is useful to you too.

## The problem

You just created an API key and your agent needs it. Today that means pasting it into the chat,
or into a `.env` the agent can `cat`. Either way the key ends up in a transcript, a log or a commit.

Keyfold splits the job in two:

| | Who handles it | Where it lives |
|---|---|---|
| **The value** | Only you, once, in a native window | macOS Keychain / Windows Credential Manager |
| **The name** (`EXA_API_KEY`) | You and your agent | A small registry of names and projects |

Your agent creates the slot, wires it into the project and runs the app. It never receives the value:
not in tool results, not in command output, not in logs.

## How it works

<p align="center">
  <img src="docs/images/flow.svg" width="900" alt="Flow: the agent asks Keyfold for a slot, you paste the value in a native window, Keyfold stores it in the OS credential store and returns only a reference; at run time the value is injected into your app's environment and masked in its output">
</p>

A typical session:

> **You:** I've got the Exa key for the search feature.
>
> **Agent:** calls `create_secret("EXA_API_KEY", project: "search-bot")`
>
> **Keyfold:** opens this window. You paste, press Enter, done.

<p align="center">
  <img src="docs/images/paste-window.png" width="420" alt="Paste window for STRIPE_SECRET_KEY: a masked field, a warning to only paste keys you asked for, Cancel and Save">
</p>

> **Agent:** gets `keyfold://EXA_API_KEY`, then starts your app with
> `keyfold run --project search-bot -- npm run dev`.
> Your app reads `process.env.EXA_API_KEY` as usual. If anything prints the key, the output shows `[keyfold:EXA_API_KEY]`.

### Working with a team

Keyfold has no cloud and no accounts, on purpose. Each person keeps their own keys on their own machine:

1. Someone gets the key (from the provider, or a teammate sends it over a channel you trust).
2. They tell their agent "I have it". The paste window opens on their machine.
3. The project only refers to the key by name, so the same code works for everyone. Each teammate's
   Keyfold injects their own copy.

No key in the repo, no key in the chat, no `.env` to pass around.

### In the app

- **New key** creates a key by hand and opens the paste window, no agent needed.
- **Show** displays a stored value to you. On macOS, the first time after you open the window (and again each
  day) it asks for Touch ID or your password; Windows does not ask yet (see [SECURITY.md](SECURITY.md)). The value hides after 30 seconds, and while it is shown
  the window is left out of screenshots. Your agent has no way to do this: there is no CLI command
  or MCP tool for it.

## Install

**macOS (Apple Silicon):** download the signed and notarized DMG from the
[latest release](https://github.com/diegoperezg7/keyfold/releases/latest) and drag Keyfold to Applications.

**Other platforms, or from source.** You need:

- [Rust](https://rustup.rs) (stable) and Node.js 22+
- macOS: the Xcode Command Line Tools (`xcode-select --install`)
- Windows: the Microsoft C++ Build Tools and WebView2 (preinstalled on Windows 11)

```sh
git clone https://github.com/diegoperezg7/keyfold.git
cd keyfold/desktop
npm ci
npx tauri build --bundles app       # macOS: target/release/bundle/macos/Keyfold.app
# npx tauri build --bundles nsis    # Windows: target/release/bundle/nsis/Keyfold_*-setup.exe
```

On macOS, move the app to Applications (`ditto target/release/bundle/macos/Keyfold.app /Applications/Keyfold.app`).

The same binary is the app, the CLI and the MCP server. Link it into your `PATH` so `keyfold` works in
every shell, including your agent's (an `alias` is not enough: non-interactive shells ignore it):

```sh
sudo ln -sf /Applications/Keyfold.app/Contents/MacOS/keyfold /usr/local/bin/keyfold
keyfold list
```

If you skip this, the MCP server tells your agent the binary's full path, so `keyfold run` still works for it.

Open it once (`open -a Keyfold`). It stays in the menu bar while it runs, with quick access to
the keys that are waiting for a value, a new key, and connecting your agent. Closing the window
does not quit it; use **Quit Keyfold** in that menu.

### Updates

A Keyfold installed from a release DMG updates itself. When a newer release is out, the keys
window shows it in a corner: **Update** downloads it with a progress bar, checks its signature,
installs it and restarts. Back in the app, a window confirms the install and lists what changed.
Builds made from source do not update themselves; pull and build again.

### Signed DMG (maintainers)

`desktop/scripts/release-macos.sh` builds the app and a drag-to-install DMG, signs both with your
Developer ID and, if you give it a notarytool profile, notarizes and staples the DMG.
It also signs the update archive with the updater key and writes the `latest.json` that installed
copies read from the latest GitHub release. Credentials stay in your keychain or home folder;
nothing secret goes in the repo.

```sh
xcrun notarytool store-credentials keyfold-notary --apple-id you@example.com --team-id TEAMID   # once
npx tauri signer generate --ci -w ~/.tauri/keyfold-updater.key                                  # once; back it up
APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
KEYFOLD_NOTARY_PROFILE=keyfold-notary \
KEYFOLD_RELEASE_NOTES=notes.txt \
./scripts/release-macos.sh
```

Bump `version` in `src-tauri/tauri.conf.json` first, then publish with the `gh release create`
command the script prints. The updater only accepts archives signed with the key whose public
half is in `tauri.conf.json`: lose the private key and installed copies stop updating.

<p align="center">
  <img src="docs/images/dmg-window.png" width="560" alt="Keyfold installer window: drag the app to Applications">
</p>

## Connect your agent

Keyfold speaks [MCP](https://modelcontextprotocol.io) over stdio, so any MCP client can use it.
One command registers it (or **Connect agent** in the menu bar). It writes the path to the binary
and nothing else, and running it twice is harmless:

```sh
keyfold connect claude    # runs `claude mcp add --scope user keyfold ...`
keyfold connect codex     # adds [mcp_servers.keyfold] to ~/.codex/config.toml
keyfold connect cursor    # adds "keyfold" to ~/.cursor/mcp.json
```

Restart the agent afterwards. Your agent can run this itself too: it is a setup step, it never
touches a key. To do it by hand instead:

**Claude Code**

```sh
claude mcp add --scope user keyfold -- /Applications/Keyfold.app/Contents/MacOS/keyfold mcp
```

**Codex** (`~/.codex/config.toml`)

```toml
[mcp_servers.keyfold]
command = "/Applications/Keyfold.app/Contents/MacOS/keyfold"
args = ["mcp"]
```

**Cursor** (`~/.cursor/mcp.json`, or `.cursor/mcp.json` in one project)

```json
{ "mcpServers": { "keyfold": { "command": "/Applications/Keyfold.app/Contents/MacOS/keyfold", "args": ["mcp"] } } }
```

### MCP tools

None of them returns a value. They return the name, the reference, whether a value is stored, and the projects.

| Tool | What it does |
|---|---|
| `list_secrets` | List keys (optionally for one project) |
| `create_secret` | Create a slot, assign it to a project, open the paste window. Waits until you save or cancel |
| `request_secret_value` | Open the paste window to add or replace the value of an existing key |
| `assign_secret` | Make an existing key available to a project |
| `unassign_secret` | Take a key out of a project, e.g. a misspelled one |
| `link_folder` / `unlink_folder` | Link a local folder to a project on this machine, or remove the link |

## Commands

| Command | What it does |
|---|---|
| `keyfold` | Open the app |
| `keyfold create NAME [--project P] [--no-prompt]` | Create a slot and open the paste window |
| `keyfold receive NAME` | Receive mode: create the slot if missing and open the paste window |
| `keyfold set NAME [--stdin]` | Store or replace a value (window, or stdin) |
| `keyfold assign NAME --project P` | Make a key available to a project |
| `keyfold unassign NAME --project P` | Take a key out of a project (the key and its value stay) |
| `keyfold list [--project P]` | Names, status and projects. Never values |
| `keyfold run [--project P] -- <command>` | Run with the project's keys in the environment, output masked. Without `--project`, uses the nearest `.keyfold` or linked folder |
| `keyfold link [FOLDER] --project P` | Link a folder (default: the current one) to a project, on this machine only |
| `keyfold unlink [FOLDER]` / `keyfold folders` | Remove a folder link / list linked folders |
| `keyfold delete NAME` | Delete a key and its value |
| `keyfold mcp` | MCP server over stdio |
| `keyfold connect claude\|codex\|cursor` | Register Keyfold as the agent's MCP server |

To tie a folder to its project, put a `.keyfold` file at the repo root with the project name on one line
(lines starting with `#` are comments). `keyfold run -- npm run dev` then works anywhere inside it.
The file holds a name only, never a value, so it can be committed and the whole team shares it.
If the project does not exist in your Keyfold, `run` warns and starts the command without keys.

To link a folder without adding a file to the repo, run `keyfold link --project P` inside it (or let
your agent call `link_folder`). The link lives in your `registry.json` and only works on your machine.
Going up from the current folder, the nearest `.keyfold` or link wins; at the same level, the file wins.
In the app, choosing a project in the filter shows its linked folders, and you can unlink them there.

## Security

What Keyfold guarantees, and what it doesn't. The full review is in [SECURITY.md](SECURITY.md).

| Keyfold protects against | Keyfold does **not** protect against |
|---|---|
| Keys pasted into chats, transcripts and agent context | A process started by `keyfold run` reading its own environment and sending the key elsewhere |
| Keys in `.env` files, commits and screenshots | Malware running as your user |
| Keys printed to the terminal by your app (masked) | Encoded or partial copies of a key in output (only the exact value is masked) |
| The key lingering in your clipboard after you paste it (macOS) | Clipboard history apps that already recorded it |
| Typing meant for another app being saved by accident (Enter only saves pasted text) | A tricked agent asking you for a key you didn't request (the window warns you; cancel it) |

Where things are stored:

- **Values:** the OS credential store (macOS login Keychain, Windows Credential Manager), encrypted at rest and tied to your user account.
- **Names, projects and linked folders:** `registry.json` in your user data folder, readable only by you. It never contains values.

## Project layout

```
desktop/                    the app (Tauri 2)
  crates/keyfold-core/      storage, registry, masking: plain Rust, no UI
  src-tauri/                one binary: app + CLI + MCP server
  src/                      UI: React, TypeScript, Zustand
Sources/, Tests/            the original native macOS prototype (Swift)
docs/                       images and the Swift prototype notes
```

## Development

```sh
cd desktop
npm test                    # UI logic and paste-window state
cargo test --workspace      # core + CLI/MCP contract against the real credential store
npx tauri dev               # run the app with hot reload
```

The contract tests drive the real binary the way an agent would (CLI and MCP) and fail if any output contains a stored value.
