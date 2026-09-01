#!/bin/sh
# Installs the agent-rules binary for this platform from a GitHub release.
# Strict POSIX sh (dash-compatible, no bashisms) so it runs unmodified
# both as `sh install.sh` and piped straight from curl, on Linux, macOS,
# and Windows under Git Bash / MSYS (WSL reports as Linux, so it's
# already covered by that branch).
set -eu

REPO="crsanti/agent-rules"

os_name="$(uname -s)"
arch_name="$(uname -m)"

asset=""
case "$os_name" in
  Darwin)
    case "$arch_name" in
      x86_64) asset="agent-rules-darwin-amd64" ;;
      arm64) asset="agent-rules-darwin-arm64" ;;
    esac
    ;;
  Linux)
    case "$arch_name" in
      x86_64) asset="agent-rules-linux-amd64" ;;
    esac
    ;;
  MINGW*|MSYS*|CYGWIN*)
    case "$arch_name" in
      x86_64) asset="agent-rules-windows-amd64.exe" ;;
    esac
    ;;
esac

if [ -z "$asset" ]; then
  echo "agent-rules: install: no prebuilt binary for $os_name-$arch_name" >&2
  echo "agent-rules: install: supported: darwin-x86_64, darwin-arm64, linux-x86_64, windows-x86_64 (Git Bash/MSYS/Cygwin)" >&2
  echo "agent-rules: install: build from source instead: mise run build" >&2
  exit 1
fi

case "$asset" in
  *.exe) bin_name="agent-rules.exe" ;;
  *) bin_name="agent-rules" ;;
esac

if command -v curl >/dev/null 2>&1; then
  downloader="curl"
elif command -v wget >/dev/null 2>&1; then
  downloader="wget"
else
  echo "agent-rules: install: requires curl or wget on PATH" >&2
  exit 1
fi

version="${AGENT_RULES_VERSION:-}"
if [ -n "$version" ]; then
  version="${version#v}"
  url="https://github.com/$REPO/releases/download/v$version/$asset"
else
  url="https://github.com/$REPO/releases/latest/download/$asset"
fi

if [ -n "${AGENT_RULES_INSTALL_DIR:-}" ]; then
  install_dir="$AGENT_RULES_INSTALL_DIR"
elif [ -n "${HOME:-}" ]; then
  install_dir="$HOME/.local/bin"
else
  echo "agent-rules: install: HOME is not set; set AGENT_RULES_INSTALL_DIR" >&2
  exit 1
fi
mkdir -p "$install_dir"

tmp_file="$install_dir/.agent-rules.tmp.$$"
dest_file="$install_dir/$bin_name"

# Runs on every exit path, success included: once `mv -f` below has moved
# tmp_file onto dest_file there is nothing left at tmp_file, so this is a
# silent no-op then. Its job is only to keep a failed download from
# leaving a stray partial file behind in the install dir.
trap 'rm -f "$tmp_file"' EXIT

echo "agent-rules install: downloading $asset..."
case "$downloader" in
  curl)
    curl -fsSL --proto '=https' --tlsv1.2 -o "$tmp_file" "$url"
    ;;
  wget)
    wget -q -O "$tmp_file" "$url"
    ;;
esac

# chmod, then mv -f onto the final name: dest_file only ever appears at
# its final path once it's fully written and executable, never partway
# through the download.
chmod 755 "$tmp_file"
mv -f "$tmp_file" "$dest_file"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    shell_name="$(basename "${SHELL:-sh}")"
    echo
    case "$shell_name" in
      zsh)
        echo "agent-rules install: $install_dir is not on your PATH. Add this to ~/.zshrc:"
        echo "  export PATH=\"$install_dir:\$PATH\""
        ;;
      bash)
        echo "agent-rules install: $install_dir is not on your PATH. Add this to ~/.bashrc:"
        echo "  export PATH=\"$install_dir:\$PATH\""
        ;;
      fish)
        echo "agent-rules install: $install_dir is not on your PATH. Run this once:"
        echo "  fish_add_path $install_dir"
        ;;
      *)
        echo "agent-rules install: $install_dir is not on your PATH. Add it in your shell's startup file."
        ;;
    esac
    echo "agent-rules install: then restart your shell."
    ;;
esac

echo
# Guarded, not a bare call: `set -e` would otherwise abort the whole
# script here if AGENT_RULES_VERSION pins a release old enough to predate
# the `version` subcommand -- the binary is already installed correctly
# at this point regardless, so a CLI mismatch on this confirmation step
# must not be reported as an install failure.
if ! "$dest_file" version; then
  echo "agent-rules install: installed at $dest_file (couldn't run '$bin_name version' to confirm -- this release may predate that subcommand)"
fi
echo
echo "Quickstart: agent-rules apply"
