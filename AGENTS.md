This is Sing-Along — a desktop music player with synced lyrics support. It began
as a fork of LRCGET and no longer tracks upstream; `git remote fork` points at
LeisureLinux/Sing-Along and there is no `origin`.

Stack: Tauri v2 (Rust backend + Vue 3 frontend).

To help you quickly understand the project architecture:

- For the frontend (Vue), see `src/ARCHITECTURE.md`.
- For the backend (Rust), see `src-tauri/ARCHITECTURE.md`.

Read these files before working on the corresponding frontend or backend tasks.

After completing your changes, update these two `ARCHITECTURE.md` files as needed.

Before reporting work as done, run all three:

```bash
npx eslint src --include=*.js --include=*.vue   # expect 0 errors
npx vitest run                                  # expect all passing
cd src-tauri && cargo test                      # expect all passing
```

`cargo check` alone is not enough — a change that compiles can still break the
backend tests, and both suites are fast enough (< 3 s combined) that skipping
them costs more in follow-up fixes than running them.

## Lint the workflow before pushing a tag

`actionlint` catches the whole class of mistake that has broken the last several
releases in this repo — duplicated YAML keys, PowerShell syntax in a bash step,
references to steps or outputs that do not exist. All of it is visible locally in
under a second, and none of it is visible from `cargo test`.

`actionlint` also runs `shellcheck` on every `run:` block, but only if
`shellcheck` is on PATH. Without it, shell-level errors (a stray backtick, `copy`
in a default-shell step) pass locally and fail in CI.

```bash
# one-time setup if shellcheck is missing
curl -sL -o /tmp/sc.tar.xz \
  https://github.com/koalaman/shellcheck/releases/download/v0.10.0/shellcheck-v0.10.0.linux.x86_64.tar.xz
tar xf /tmp/sc.tar.xz -C /tmp && install -m755 /tmp/shellcheck-v0.10.0/shellcheck ~/.local/bin/

# before every tag
PATH="$HOME/.local/bin:$PATH" actionlint .github/workflows/publish.yml
```

The workflow itself also has a `lint` job as the first step, so CI catches it —
but that costs a full run to learn something a local second would have told you.

## Releasing: never without explicit approval

**Do not tag, push a version tag, or trigger the release workflow unless the user
has said to release in this conversation.** Commit locally, run the tests, then
stop and report — wait for a go-ahead.

Why this is a hard rule rather than a preference: a release pushes a tag that
runs the CI matrix, and the waiting (polling for ~10 minutes, then checking the
assets) is the expensive part. Shipping three patch releases in a row because
each one looked small at the time is the failure mode to avoid. Batch fixes and
let the user decide when a version goes out.

| Action | Approval needed |
|---|---|
| Edit code, run lint/tests, `git commit` | No — just do it |
| `git push` to main | Ask first |
| `git tag v*` and push it (triggers CI release) | **Only when the user says release** |
| Deleting or re-tagging an existing release | Ask first; never as a "cleanup" |

When you do cut a release, keep the three version fields in sync —
`src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` (the `sing-along` package entry),
and `src-tauri/tauri.conf.json` — and check `git status` is clean first. A
release once shipped with a feature left uncommitted because the tag was created
before the working tree was inspected.

## Inspecting a running dev build (W3C WebDriver)

`npm run tauri:dev` compiles with `--features dev,devtools`, which enables a W3C WebDriver server on `http://127.0.0.1:4445` (via `tauri-plugin-webdriver`, backend details in `src-tauri/ARCHITECTURE.md`); drive it with any WebDriver client (Selenium, WebdriverIO, raw curl). Works on X11 and any Wayland compositor — no extra tools needed. Release builds have no server (`npm run tauri build` does not enable the feature), so a WebDriver-free binary is the default.

**`tauri dev` does not enable these on its own** — it only passes `--no-default-features`. Use `npm run tauri:dev`, or add `--features dev` yourself; otherwise a debug build has no WebDriver bridge at all.

```bash
setsid npm run tauri:dev </dev/null > /tmp/lrcget-dev.log 2>&1 & disown
while ! curl -sf http://127.0.0.1:4445/status >/dev/null; do sleep 5; done
```

```python
from selenium import webdriver
from selenium.webdriver.common.by import By
driver = webdriver.Remote(command_executor="http://127.0.0.1:4445",
                          options=webdriver.ChromeOptions())  # options ignored
driver.get("http://localhost:1420")   # dev-mode URL (Vite); webview is usually already here
driver.find_element(By.CSS_SELECTOR, ".my-button").click()
driver.save_screenshot("/tmp/lrcget.png")          # full webview PNG
print(driver.execute_script("return document.title"))
driver.quit()                                       # always end the session
```
