# The web launcher

Horizon's launcher in the browser — the browser edition of what [the fork](../../README.md#this-fork)
adds, sharing the desktop launcher's design tokens ([`horizon-theme`](../../crates/horizon-theme)),
its pins ([`research-lock.json`](../../research-lock.json)) and its checks, on egui's glow
backend (WebGL2): the same UI runs on the oldest machines' browsers.

Three pages:

- **Home** — what the fork is, and where it plays.
- **Downloads** — this fork's own releases from GitHub, the platform build this device wants
  marked with *this device*.
- **Minecraft** — the pinned Minecraft 26.3:
  - *Download directly*, as the desktop launcher's does it, for the files Mojang lets a
    browser have: the version file, the client jar, the logging config, the asset index —
    every one checked against the sha1 the pin holds or the version file lists, written
    whole (a partial file never takes its place), the finished files kept when a download
    is stopped and started again. Mojang's other hosts (`libraries.minecraft.net`,
    `resources.download.minecraft.net`) answer no cross-origin request, so the libraries
    and the sounds stay for the desktop launcher's *Download directly*, which finds what
    the browser already put in the folder (launcher/src/sources.rs).
  - *Verify*: the folder's three pinned files, each compared with the pin.

The folder is the player's own, chosen with the browser's File System Access API
(Chromium; Firefox and Safari get the pages without the folder), and nothing leaves the
browser: downloads go straight from Mojang, the sha1s are computed by the browser's own
Web Crypto.

## Building it

```sh
cargo run -p horizon-web                            # the pages, native: every browser
                                                    # power answers a note
cargo test -p horizon-web                           # the plan and the pins' checks
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript \
  --out-dir out target/wasm32-unknown-unknown/release/horizon_web.wasm
cp index.html out/                                   # the site: three files
```

The pins are baked at build time (build.rs reads `research-lock.json`; the repository and
version from `HORIZON_GAME_REPO` and `HORIZON_LAUNCHER_VERSION`, as the desktop launcher's
build bakes its own), and the workflow builds and serves it
([.github/workflows/build.yml](../../.github/workflows/build.yml), the `web` job): as
`horizon-launcher-web.zip` on every `launcher@X.Y.Z` release, and on this fork's GitHub
Pages.

Two languages, English and Indonesian; every string lives in both, side by side
([`src/lang.rs`](src/lang.rs)), so a missing one cannot compile away.
