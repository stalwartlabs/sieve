# Sievepad

Sievepad (https://sievepad.com) is a browser-only playground for Sieve email
filters. It compiles and runs scripts with `sieve-rs` compiled to WebAssembly,
the same interpreter used by [Stalwart Mail Server](https://stalw.art). Scripts,
test messages and settings never leave the browser: workspaces are stored in
IndexedDB, and shared links carry the workspace compressed in the URL fragment.

This directory is a standalone crate (its own workspace) that depends on the
parent crate by path. It is never published.

## Layout

- `src/lib.rs`: `wasm-bindgen` exports: `version`, `capabilities`, `defaults`,
  `compile` and `run`.
- `src/settings.rs`: every compiler and runtime option, with permissive defaults.
- `src/run.rs`: compiles the main script and include tabs, runs one message and
  collects the result.
- `src/handler.rs`: the `Handler` that answers `mailboxexists`, extlists and
  `duplicate` from the settings and turns actions into events.
- `src/output.rs`: diagnostics, events and parsed output messages.
- `src/functions/`: the expression functions available to untrusted scripts in
  Stalwart (`trim`, `len`, `email_part`, ...).
- `src/tests.rs`: runs every sample and checks error reporting.
- `site/`: static HTML, CSS and ES modules. `worker.js` runs the wasm module off
  the main thread; `sieve-language.js` holds the Monarch grammar, completions,
  hovers, quick fixes and formatter; `result.js` renders the result pane.
- `site/samples/`: example workspaces listed in `index.json`. The first visit
  opens `welcome`.

## Build

```sh
cargo install wasm-pack   # once
./web/build.sh            # outputs web/dist/
python3 -m http.server --directory web/dist 8080
```

`build.sh` downloads Monaco 0.52.2 from the npm registry into `web/vendor/` on
the first run (verifying its SHA-512) and copies only the editor core into
`dist/vendor/monaco-0.52.2/`. Everything else goes into
`dist/assets/<content hash>/`, and `dist/index.html` is generated from
`site/index.html` with `__ASSETS__` replaced by that path. A deploy therefore
never mixes files from two builds, even when a CDN or browser caches old
scripts. When bumping Monaco, update the version in both `build.sh` and
`site/index.html`; the build fails if they disagree. Version 0.52.2 is
the last release whose AMD build is supported and loads languages lazily, which
matters because the site has no bundler.

Run the tests with `cargo test` inside `web/`.

## Deploy

Pushes to `main` that touch `web/`, `src/` or `Cargo.toml` run
`.github/workflows/pages.yml`, which tests, builds and publishes `web/dist` to
GitHub Pages. `site/CNAME` keeps the `sievepad.com` custom domain across
deploys.
