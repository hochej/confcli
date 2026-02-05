#!/bin/sh
# Install script for confcli
# Usage: curl -fsSL https://raw.githubusercontent.com/hochej/confcli/main/install.sh | sh
#
# Options (via env vars):
#   INSTALL_DIR  — where to put the binary (default: /usr/local/bin or ~/.local/bin)
#   VERSION      — specific version to install (default: latest)

set -e

REPO="hochej/confcli"
BINARY="confcli"

# --- helpers ----------------------------------------------------------------

info()  { printf '  \033[1;34m→\033[0m %s\n' "$1"; }
ok()    { printf '  \033[1;32m✓\033[0m %s\n' "$1"; }
err()   { printf '  \033[1;31m✗\033[0m %s\n' "$1" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || err "Required tool '$1' not found. Please install it and retry."
}

# --- detect platform --------------------------------------------------------

detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux"  ;;
        Darwin*) echo "macos"  ;;
        *)       err "Unsupported OS: $(uname -s). confcli provides binaries for Linux and macOS." ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)       echo "amd64" ;;
        aarch64|arm64)      echo "arm64" ;;
        *)                  err "Unsupported architecture: $(uname -m). confcli provides binaries for amd64 and arm64." ;;
    esac
}

# --- resolve version --------------------------------------------------------

resolve_version() {
    if [ -n "$VERSION" ]; then
        # Strip leading 'v' if the user passed it, we'll add it back
        echo "$VERSION" | sed 's/^v//'
        return
    fi

    info "Fetching latest release…"
    tag=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"//;s/".*//' \
        | sed 's/^v//')

    [ -z "$tag" ] && err "Could not determine latest version. Set VERSION=x.y.z and retry."
    echo "$tag"
}

# --- choose install dir -----------------------------------------------------

choose_install_dir() {
    if [ -n "$INSTALL_DIR" ]; then
        echo "$INSTALL_DIR"
        return
    fi

    if [ -w /usr/local/bin ]; then
        echo "/usr/local/bin"
    else
        echo "$HOME/.local/bin"
    fi
}

# --- main -------------------------------------------------------------------

main() {
    need curl
    need tar

    printf '\n\033[1m  confcli installer\033[0m\n\n'

    OS=$(detect_os)
    ARCH=$(detect_arch)
    VER=$(resolve_version)
    DIR=$(choose_install_dir)
    ASSET="${BINARY}-${OS}-${ARCH}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/v${VER}/${ASSET}"

    info "Platform:  ${OS}/${ARCH}"
    info "Version:   v${VER}"
    info "Install:   ${DIR}/${BINARY}"
    echo ""

    # Download
    info "Downloading ${ASSET}…"
    TMPDIR_DL=$(mktemp -d)
    trap 'rm -rf "$TMPDIR_DL"' EXIT
    HTTP_CODE=$(curl -fSL -w '%{http_code}' -o "${TMPDIR_DL}/${ASSET}" "$URL" 2>/dev/null) || true

    [ "$HTTP_CODE" != "200" ] && err "Download failed (HTTP ${HTTP_CODE}). Check that v${VER} exists at:\n         ${URL}"

    # Extract
    tar -xzf "${TMPDIR_DL}/${ASSET}" -C "$TMPDIR_DL"
    [ ! -f "${TMPDIR_DL}/${BINARY}" ] && err "Archive did not contain '${BINARY}' binary."
    chmod +x "${TMPDIR_DL}/${BINARY}"

    # Install
    mkdir -p "$DIR"
    if [ -w "$DIR" ]; then
        mv "${TMPDIR_DL}/${BINARY}" "${DIR}/${BINARY}"
    else
        info "Elevated permissions required for ${DIR} — running sudo mv …"
        sudo mv "${TMPDIR_DL}/${BINARY}" "${DIR}/${BINARY}"
    fi

    ok "Installed confcli v${VER} to ${DIR}/${BINARY}"

    # PATH check
    case ":$PATH:" in
        *":${DIR}:"*) ;;
        *)
            echo ""
            printf '  \033[1;33m⚠\033[0m  %s is not in your PATH.\n' "$DIR"
            printf '     Add it with:\n'
            printf '       export PATH="%s:$PATH"\n' "$DIR"
            ;;
    esac

    echo ""
    ok "Run 'confcli --help' to get started."
    echo ""
}

main
