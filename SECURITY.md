# Security

Keyfold's job is narrow: a key you paste must reach the OS credential store and your app's
environment, and nothing else. In particular it must never reach your coding agent: not in tool
results, command output, logs or files.

This document is the result of a review of the desktop app (`desktop/`). Findings are ordered by
severity. Each one says whether it is **fixed**, **mitigated** or **open**.

## Threat model

- **Trusted:** you, your OS user account, the OS credential store.
- **Not trusted:** the coding agent and anything it reads (it can be prompt-injected), and the
  output of programs it runs.
- **Out of scope:** malware running as your user, or a compromised OS. Such code can read the
  credential store the same way Keyfold does.

## Findings

| ID | Severity | Finding | Status |
|---|---|---|---|
| H1 | High | The paste window took focus while you were typing in another app. Your keystrokes landed in the field and the Enter at the end of your message saved them as the key. Seen twice in manual testing. | **Fixed.** Enter only saves a value that arrived by pasting. Typed text needs a click on Save. Covered by a test. |
| H2 | High | A program started with `keyfold run` receives the real key in its environment. If the agent controls that program, it can send the key elsewhere or print it encoded. Masking only recognizes the exact bytes of the value: base64, hex, URL-encoded, truncated or split copies pass through unmasked. | **Open, by design.** Documented. Masking is a guard against accidents, such as a debug line or a stack trace, not against a program that wants the key out. A future proxy mode, where the app gets a placeholder and a local proxy adds the real header, would close it for HTTP APIs. |
| H3 | High | A prompt-injected agent can call `create_secret` with a convincing name (`GITHUB_TOKEN`) to get you to paste a key it should not have, then use `keyfold run` to read it. | **Mitigated.** The paste window warns: "only paste keys you asked for". Open: show which agent and project asked, and require confirmation for unusual names. |
| H4 | High | A value containing line breaks could be printed partly unmasked by `keyfold run`: output is masked as it streams, and the part of the value before a chunk boundary was written before the rest arrived. | **Fixed.** The masker holds back a tail as long as the longest stored value until it knows the tail is not the start of a value, so a value split across reads is still replaced. |
| M1 | Medium | After you paste, the key stays in the clipboard, where any process running as your user can read it (`pbpaste`). | **Fixed on macOS:** after saving, Keyfold clears the clipboard if it still holds that exact value. Test on the real clipboard. **Open on Windows.** Clipboard history apps may already have recorded it. |
| M2 | Medium | The UI had Tauri's `core:default` permission set, wider than it needs. | **Fixed.** Reduced to `core:event:default`. App commands return names and status only, except the reveal button (M6). |
| M3 | Medium | Builds were not signed with a Developer ID or notarized. macOS ties Keychain access to each build, so every rebuild asked again. Users could not verify where a downloaded build came from. | **Fixed.** `npm run dmg` (`desktop/scripts/release-macos.sh`) signs the app and the DMG with a Developer ID and the hardened runtime, and notarizes and staples the DMG with the maintainer's notarytool profile by default (`KEYFOLD_NOTARY_PROFILE=` skips it). Credentials stay in the maintainer's keychain. |
| M6 | Medium | The keys window can show a stored value (Show button). Anyone at your unlocked Mac could read a key, and a screenshot taken at that moment would capture it. | **Mitigated.** Only in the keys window and only on a click: refused in the paste window, and the CLI and MCP server have no equivalent. On macOS the first reveal after the window opens asks for Touch ID or the account password; the unlock ends when the window closes, the app quits or the day changes, and it lives only in memory, so no file can fake it. The value hides after 30 seconds and, while shown, the window is left out of screen capture. **On Windows, turned off:** without a Windows Hello prompt, Show is refused until that prompt exists. |
| M7 | Medium | Sharing reads values to encrypt them. An agent with a shell could run `keyfold share --all --to <its own key>` and walk away with every key. | **Mitigated.** Sharing asks for Touch ID or the account password every time, in the CLI and the app; there is no MCP tool for it. Packages are age (X25519) files: only the listed recipients can open them, and each install's private key stays in its own credential store. Importing needs no prompt: it can only do what `keyfold set --stdin` already can (see M4). **On Windows, turned off:** without a Windows Hello prompt, sharing is refused in the CLI and the app until that prompt exists. |
| M4 | Medium | The agent can replace a value with `keyfold set NAME --stdin`, or remove one with `keyfold delete`. It cannot read values. It could swap your key for one it controls, or break your setup. | **Open.** Candidates: require the paste window for `set` on real stores, and confirmation for `delete`. |
| M5 | Medium | A Keychain item can only be updated or deleted by the binary that created it (`-25244`). Keys created by another build (or by the Swift prototype) cannot be changed from this one. | **Partly fixed.** Delete retries with Apple's `/usr/bin/security` tool, which can remove such items (macOS may ask first). **Open for updates:** replacing the value still fails; it fails closed, no data is exposed. Workaround: delete the key and paste it again. |
| M8 | Medium | A repository you clone can carry a `.keyfold` file. If it names a project that exists in your Keyfold, `keyfold run` in that repository injects that project's keys into whatever its scripts do (`npm run dev`, `make`), and those scripts are third-party code. | **Open.** Before using `keyfold run` in a repository you did not write, read its `.keyfold` (or pass `--project` explicitly). Candidates: ask for confirmation the first time a `.keyfold` is used in a new folder. |
| L1 | Low | The registry folder was created world-readable (0755). It holds key and project names, not values. | **Fixed.** Folder 0700, file 0600. Covered by a test. |
| L2 | Low | Every save used the same temp file name. The app, the MCP server and the CLI saving at once could clobber each other's write. | **Fixed.** Unique temp file per process and write. |
| L3 | Low | Two simultaneous load-modify-save cycles can lose one change, such as a project assignment. | **Fixed.** Every load-modify-save of the registry holds a file lock, so concurrent writers wait for each other. No values are involved. |
| L4 | Low | The paste window reports "saved" through a file in your per-user temp folder. A process running as you could fake that status. | **Open.** It only affects the status shown; the value itself is never in the file. |
| L5 | Low | While you type, the value lives in the web view's memory as a JavaScript string, which cannot be wiped on demand. | **Accepted.** It is cleared from state on save or cancel, and the window closes right after. |
| L6 | Low | Windows: the binary uses the console subsystem (needed for CLI and MCP), so the app opens a console window. | **Open.** On Windows, CI runs the Credential Manager round trip and the MCP contract test, and builds the installer. The UI has not been tried by hand on Windows yet. |

## Verified properties

- **No value in any CLI or MCP output.** The contract tests in `desktop/src-tauri/tests/agent_contract.rs`
  drive the real binary against the real credential store and check every output. They include a child
  process that tries to print the key on both streams. Disabling masking makes them fail, so they would
  catch a regression.
- **Values only in the credential store.** The registry file is checked for values in tests.
- **Paste window state.** After saving or cancelling, the UI state holds no trace of the value. Typed
  text followed by Enter does not save (`desktop/src/stores/prompt.test.ts`).
- **UI hardening.** Strict CSP, no remote content, DevTools off in release builds, least-privilege capability.
- **Dependencies.** CI runs `npm audit --omit=dev --audit-level=high` and `cargo audit` on every change.
  At the last review both reported 0 vulnerabilities; `cargo audit` lists 4 warnings in transitive
  dependencies of Tauri (3 unmaintained crates and `glib`, which only builds on Linux).

## Reporting a vulnerability

Please use GitHub's **Report a vulnerability** (Security tab → Advisories) rather than a public issue.
