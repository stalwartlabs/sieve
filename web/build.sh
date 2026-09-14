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

MONACO_PATH="vendor/monaco-$MONACO_VERSION"
if ! grep -q "\./$MONACO_PATH/vs/loader.js" site/index.html; then
  echo "site/index.html does not reference $MONACO_PATH; update it together with MONACO_VERSION." >&2
  exit 1
fi

STAGE_DIR="target/site-stage"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

wasm-pack build --release --target web --out-dir "$STAGE_DIR/pkg" --no-typescript
rm -f "$STAGE_DIR/pkg/.gitignore" "$STAGE_DIR/pkg/package.json" "$STAGE_DIR/pkg/README.md"
cp -r site/. "$STAGE_DIR/"
rm "$STAGE_DIR/index.html" "$STAGE_DIR/CNAME"

BUILD_ID="$(cd "$STAGE_DIR" && find . -type f | LC_ALL=C sort | xargs openssl dgst -sha256 | openssl dgst -sha256 | awk '{print $NF}' | cut -c1-16)"
ASSETS="assets/$BUILD_ID"

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/$ASSETS" "$OUT_DIR/$MONACO_PATH/vs"

cp "$MONACO_DIR/min/vs/loader.js" "$OUT_DIR/$MONACO_PATH/vs/"
cp -r "$MONACO_DIR/min/vs/base" "$MONACO_DIR/min/vs/editor" "$OUT_DIR/$MONACO_PATH/vs/"
cp "$MONACO_DIR/LICENSE" "$OUT_DIR/$MONACO_PATH/LICENSE"

cp -r "$STAGE_DIR/." "$OUT_DIR/$ASSETS/"
cp site/CNAME site/favicon.svg "$OUT_DIR/"
sed "s#__ASSETS__#./$ASSETS#g" site/index.html > "$OUT_DIR/index.html"
rm -rf "$STAGE_DIR"

echo "Built static site in web/$OUT_DIR (assets in $ASSETS)"
echo "Serve locally with: python3 -m http.server --directory web/$OUT_DIR 8080"
