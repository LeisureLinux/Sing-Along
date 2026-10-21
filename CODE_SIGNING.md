# Code signing policy

Free code signing for Windows builds is provided by
[SignPath.io](https://signpath.io), with the certificate issued by
[SignPath Foundation](https://signpath.org).

Windows installers (`.exe`, NSIS) and the standalone `singalong-cli` binary are
built by GitHub Actions from this repository and submitted to SignPath for
Authenticode signing. The private key is held in SignPath's hardware security
module; it is never on a build machine.

Signed Windows files show **SignPath Foundation** as the publisher. That is the
foundation's certificate, not this project's — the signature attests that the
binary was built from this repository, which is the guarantee the certificate is
designed to carry.

## What this means in practice

- `sing-along_<version>_x64-setup.exe` — signed, so SmartScreen does not show the
  "unknown publisher" prompt for a download from the release page.
- `singalong-cli-windows-amd64.exe` — signed the same way.
- Linux (`.deb`, `.rpm`, `.AppImage`) and macOS (`.dmg`) builds are not signed
  here: Linux packages carry no Authenticode equivalent, and Apple code signing
  needs a paid Developer ID that this project does not have. macOS users
  right-click the app and choose Open, or clear the quarantine flag:

  ```bash
  xattr -dr com.apple.quarantine /Applications/Sing-Along.app
  ```

## Build process

The workflow builds on GitHub-hosted runners, uploads the Windows artifacts,
submits them to SignPath as one signing request, and only then attaches the signed
files to the release. Nothing unsigned reaches a release page.

## Roles

This is a single-maintainer project. Releases are approved by the maintainer
before a tag is pushed; the tag is what triggers the build and the signing
request.

---

Last updated: see the repository's commit history for this file.
