# Submitting to Flathub

The Flatpak is built by Flathub from the manifest in this directory. Nothing is
published to the store from this repository — the bundle on the GitHub release
page is a convenience for people who cannot use Flathub, and is not what the
store serves.

The files Flathub needs are:

| File | What it is |
| --- | --- |
| `com.freelamp.SingAlong.yml` | the manifest |
| `com.freelamp.SingAlong.metainfo.xml` | AppStream metadata |
| `com.freelamp.SingAlong.desktop` | the desktop entry |
| `com.freelamp.SingAlong.png` | the icon, at least 128×128 |

## The submission has to be made by hand

Flathub's generative-AI policy says:

> AI tools or agents must not open or automate Flathub submission pull requests,
> or generate their commit messages, descriptions, review comments, or replies.

and

> Flathub manifests must not contain AI-generated or AI-assisted content.

So the pull request, its commit message, its description, and the replies to the
reviewer have to be written by a person. The files above are drafts: read them,
change what you would write differently, and write the prose yourself.

The checklist in the pull request template asks whether AI tools were used to
generate the submission. Answer it as it is. The policy requires disclosure of
AI-generated material in the application and its packaging, and the reviewer
decides what to do with it — concealment is the thing that gets a project
banned, not disclosure.

## What the manifest points at

`com.freelamp.SingAlong.yml` names a published release asset and its checksum.
Both go stale with every version, so before opening a pull request:

```bash
# 1. build the release, so the asset exists
#    (push the tag, wait for the release workflow)

# 2. update the three values in the manifest
VERSION=2.9.30
DEB=sing-along_${VERSION}_amd64.deb
URL=https://github.com/LeisureLinux/Sing-Along/releases/download/v${VERSION}/${DEB}
SHA=$(curl -sL "$URL" | sha256sum | cut -d' ' -f1)

sed -i "s|releases/download/v[0-9.]*/${DEB}|releases/download/v${VERSION}/${DEB}|" \
  com.freelamp.SingAlong.yml
sed -i "s|sha256: [0-9a-f]*|sha256: $SHA|" com.freelamp.SingAlong.yml
```

Check the result before using it:

```bash
appstreamcli validate --no-net com.freelamp.SingAlong.metainfo.xml
desktop-file-validate com.freelamp.SingAlong.desktop
```

The CI workflow builds the manifest on every tag, which is the check worth
trusting: it produces the same bundle Flathub will produce, and reads it back
with `flatpak remote-info`.

## Opening the pull request

```bash
gh repo fork flathub/flathub --clone=false
git clone --branch new-pr https://github.com/<you>/flathub.git
cd flathub
git checkout -b add-com.freelamp.SingAlong
cp <this directory>/com.freelamp.SingAlong.* .
git add com.freelamp.SingAlong.*
git commit -m "..."
git push -u origin add-com.freelamp.SingAlong
gh pr create --repo flathub/flathub \
  --base new-pr --head <you>:add-com.freelamp.SingAlong \
  --title "Add com.freelamp.SingAlong" --body "..."
```

The base branch is `new-pr`, not `master`. A pull request against `master` is
closed automatically.

## What the checklist asks for

- A description of the application.
- **A video of the Flatpak running on Linux.** The application id has to be
  verified this way, so there is no way around it.
- The application id follows the rules: three or more components, the domain
  first, and it must resolve to a domain the project controls.
- The submission requirements have been read and accepted, including:
  - a meaningful development history, evidence of real-world use, and a
    commitment to ongoing maintenance;
  - disclosure of any AI-generated material;
  - a statement about whether AI tools were used for the pull request itself.
- A statement that the submitter is the author, developer or upstream
  contributor, or has contacted them.

The history requirement is the one that decides whether to apply at all:

> Applications that have only existed for a very short period of time will
> generally not be accepted.

A project with a handful of downloads and no users is not ready. The upstream
project this was forked from has existed since 2022 and is worth citing, but the
fork's own history starts when the fork does.

## What the reviewer will ask about

- **The app id.** `com.freelamp.SingAlong` needs `freelamp.com` to be
  verifiable, which it is: <https://singalong.freelamp.com> resolves and points
  at the project's own page.
- **The permissions.** `--filesystem=xdg-music` and `--filesystem=xdg-download`
  rather than `--filesystem=home`, which Flathub rejects as too broad. The file
  chooser portal covers other locations for the length of a session.
- **The tray icon.** GNOME does not provide libayatana-appindicator3, and
  neither does the Flatpak runtime, so the application has no tray icon there
  and closing the window quits it. That is stated in the metadata.
- **The build.** Building from the published .deb rather than from source is
  deliberate: compiling a Tauri application inside the SDK means building
  webkit2gtk, a Rust toolchain and the whole node dependency tree in the
  sandbox, for a binary the project's CI has already produced and tested. The
  reviewer may want a source build instead; that is a fair question and the
  answer is in the manifest's comments.
