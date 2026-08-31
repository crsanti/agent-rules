# syntax=docker/dockerfile:1
#
# Toolchain image for cross-compiling agent-rules. The actual project
# build does NOT happen at `docker build` time -- it runs every time the
# container starts (see /build.sh), because it needs the live, read-only
# /src bind mount that only exists at `docker compose run` time.
FROM ghcr.io/rust-cross/cargo-zigbuild:latest

ENV CARGO_HOME=/root/.cargo \
    CARGO_TARGET_DIR=/cargo-target

# musl (not gnu) for the linux target, for a genuinely static binary with
# no runtime libc dependency. windows-gnu gets an apt mingw-w64 fallback
# installed below in case cargo-zigbuild's bundled mingw sysroot has
# trouble with it (see build_one's fallback in build.sh).
RUN rustup target add \
      x86_64-unknown-linux-musl \
      x86_64-apple-darwin \
      aarch64-apple-darwin \
      x86_64-pc-windows-gnu

RUN (apt-get update && apt-get install -y --no-install-recommends mingw-w64 \
      && rm -rf /var/lib/apt/lists/*) \
    || (apk add --no-cache mingw-w64-gcc) \
    || echo "warning: could not install a mingw-w64 fallback toolchain"

COPY <<'EOF' /build.sh
#!/bin/sh
set -eu

SRC=/src
DIST=/dist

mkdir -p "$DIST"

# Cargo.toml, src/, build.rs, and blocks/ all live at the repo root as
# plain siblings, so the build runs directly against the read-only /src
# mount -- no assembly step needed. CARGO_TARGET_DIR (above) points at a
# writable cache volume, since /src itself cannot be written to; --locked
# turns any attempt to rewrite Cargo.lock into a clear error instead of a
# read-only-filesystem failure. A live bind mount also preserves real
# mtimes with no copy step involved, which is what Cargo's mtime-based
# incremental cache needs to recognize a warm, unchanged rebuild.
cd "$SRC"

echo "== toolchain =="
cargo --version
rustc --version
cargo zigbuild --version || true
echo

: > /tmp/timings.txt

build_one () {
  label="$1"
  triple="$2"
  suffix="$3"
  out="$DIST/agent-rules-$label$suffix"
  echo "==> building $label ($triple)"
  start=$(date +%s)
  if cargo zigbuild --release --locked --target "$triple"; then
    :
  else
    echo "zigbuild failed for $triple, falling back to plain cargo build"
    cargo build --release --locked --target "$triple"
  fi
  end=$(date +%s)
  elapsed=$((end - start))
  cp "$CARGO_TARGET_DIR/$triple/release/agent-rules$suffix" "$out"
  echo "$label ${elapsed}s"
  echo "$label ${elapsed}s" >> /tmp/timings.txt
}

build_one linux-amd64   x86_64-unknown-linux-musl ""
build_one darwin-amd64  x86_64-apple-darwin       ""
build_one darwin-arm64  aarch64-apple-darwin      ""
build_one windows-amd64 x86_64-pc-windows-gnu     ".exe"

echo
echo "=== timings (agent-rules) ==="
cat /tmp/timings.txt
echo
echo "=== sizes (agent-rules) ==="
ls -la "$DIST"
EOF

RUN chmod +x /build.sh

CMD ["/build.sh"]
