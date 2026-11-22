# Platform runbook: macOS and Linux

Windows is built and executed on this machine. macOS and Linux are **built and tested on
CI only**; their installed acceptance is **blocked — hardware**, and this runbook is what a
person with those machines would run. Nothing below has been executed here, and no row that
depends on it may be marked pass.

## What CI does and does not prove

CI (`native-manual.yml`) builds the desktop on `macos-15` (aarch64) and `ubuntu-24.04`
(x86_64) with `--features native`, runs the full test suite, and clippy with warnings as
errors. That is **build and test** evidence. It is not installed acceptance: no package is
created there and no window is opened.

## macOS (blocked — hardware)

Build a bundle on a Mac with Xcode command line tools:

```sh
cargo build --release -p pfl-desktop --features native --target aarch64-apple-darwin
bash packaging/macos/bundle.sh
```

`bundle.sh` assembles `Patent Forge Lite.app` from the binary, `Info.plist`, the icon and
`prompts/`. Unsigned: macOS will require right-click → Open, or `xattr -dr com.apple.quarantine`
on the app, and Gatekeeper will refuse a downloaded unsigned copy from another machine.

Checks to run on the machine, recorded with their output:

1. The app launches from `Applications` and shows the first-run profile prompt.
2. Choosing Full allows a loopback Ollama endpoint; choosing Lean refuses it.
3. A run completes against `scripts/mock-ollama.ps1`'s macOS equivalent, or a real local
   runtime, and the run summary shows the checkpoint count.
4. Data lives under `~/Library/Application Support/patent-forge-lite/`; deleting the app
   leaves it in place.
5. Reinstalling the newer build over the older one keeps that data (the Windows upgrade
   proof in `docs/evidence/upgrade/` is the model to follow).

## Linux (blocked — hardware)

```sh
cargo build --release -p pfl-desktop --features native
install -Dm755 target/release/pfl-desktop /usr/local/bin/pfl-desktop
install -Dm644 packaging/linux/pfl-desktop.desktop /usr/share/applications/pfl-desktop.desktop
```

The desktop entry assumes an X11 or Wayland session with GPU acceleration available; a
headless VM will fail to create a window and is not a valid test.

Checks to run, same shape as macOS:

1. Launches from the desktop entry, first-run prompt appears.
2. Full permits a loopback runtime, Lean refuses it.
3. A run completes and the summary reports its checkpoints.
4. Data lives under `~/.local/share/patent-forge-lite/` and survives removal of the
   package.
5. Upgrade in place from the previous build keeps the data.

## Signature status

Both platforms are **unsigned** and unnotarised. Windows binaries are also unsigned in
the candidate build (`docs/RELEASE-MANIFEST.md` records this per artifact). Signing
requires approval and a certificate that is not present here; nothing in this build
claims to be signed except the accepted v0.1.0 release, whose status is recorded
separately.

## What would unblock these rows

A macOS machine and a Linux machine with a display, each able to run the app, plus
approval to sign and notarise if that is wanted. Until then the honest row is
*blocked — hardware*, and the runbook above is the deliverable.
