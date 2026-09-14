#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

MONACO_VERSION=0.52.2
MONACO_INTEGRITY="sha512-GEQWEZmfkOGLdd3XK8ryrfWz3AIP8YymVXiPHEdewrUq7mh0qrKrfHLNCXcbB6sTnMLnOZ3ztSiKcciFUkIJwQ=="
OUT_DIR="dist"

for tool in wasm-pack curl tar openssl; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool not found." >&2
    [ "$tool" = "wasm-pack" ] && echo "Install it with: cargo install wasm-pack" >&2
    exit 1
  fi
done

MONACO_DIR="vendor/monaco-$MONACO_VERSION"
if [ ! -f "$MONACO_DIR/min/vs/loader.js" ]; then
  rm -rf "$MONACO_DIR"
  mkdir -p "$MONACO_DIR"
  MONACO_TGZ="$(mktemp)"
  trap 'rm -f "$MONACO_TGZ"' EXIT
  curl -fsSL -o "$MONACO_TGZ" "https://registry.npmjs.org/monaco-editor/-/monaco-editor-$MONACO_VERSION.tgz"
  ACTUAL_INTEGRITY="sha512-$(openssl dgst -sha512 -binary "$MONACO_TGZ" | openssl base64 -A)"
  if [ "$ACTUAL_INTEGRITY" != "$MONACO_INTEGRITY" ]; then
    rm -rf "$MONACO_DIR"
    echo "Monaco tarball integrity mismatch: expected $MONACO_INTEGRITY, got $ACTUAL_INTEGRITY" >&2
    exit 1
  fi
  tar -xzf "$MONACO_TGZ" -C "$MONACO_DIR" --strip-components=1
fi

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/vendor/monaco/vs"

wasm-pack build --release --target web --out-dir "$OUT_DIR/pkg" --no-typescript
rm -f "$OUT_DIR/pkg/.gitignore" "$OUT_DIR/pkg/package.json" "$OUT_DIR/pkg/README.md"

cp "$MONACO_DIR/min/vs/loader.js" "$OUT_DIR/vendor/monaco/vs/"
cp -r "$MONACO_DIR/min/vs/base" "$MONACO_DIR/min/vs/editor" "$OUT_DIR/vendor/monaco/vs/"
cp "$MONACO_DIR/LICENSE" "$OUT_DIR/vendor/monaco/LICENSE"

cp -r site/. "$OUT_DIR/"

echo "Built static site in web/$OUT_DIR"
echo "Serve locally with: python3 -m http.server --directory web/$OUT_DIR 8080"
