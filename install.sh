#!/bin/sh
# Install script for confcli
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/hochej/confcli/main/install.sh | sh
#
# Options (via env vars — set them on the `sh` side of the pipe):
#   curl -fsSL https://raw.githubusercontent.com/hochej/confcli/main/install.sh | INSTALL_DIR=~/.local/bin sh
#   curl -fsSL https://raw.githubusercontent.com/hochej/confcli/main/install.sh | VERSION=0.2.3 sh
#
# Env vars:
#   INSTALL_DIR  — where to put the binary (default: /usr/local/bin or ~/.local/bin)
#   VERSION      — specific version to install (default: latest)

set -e

REPO="hochej/confcli"
BINARY="confcli"

# --- helpers ----------------------------------------------------------------

info()  { printf '  \033[1;34m→\033[0m %s\n' "$1" >&2; }
ok()    { printf '  \033[1;32m✓\033[0m %s\n' "$1" >&2; }
err()   { printf '  \033[1;31m✗\033[0m %b\n' "$1" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || err "Required tool '$1' not found. Please install it and retry."
}

sha256_file() {
    # Print the SHA256 hash (hex) for a given file.
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
        return
    fi
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
        return
    fi
    err "Required tool 'sha256sum' (Linux) or 'shasum' (macOS) not found. Please install one and retry."
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

    # Avoid brittle JSON parsing (and GitHub API rate limits) by following the
    # redirect of the human "latest" URL.
    tag_url=$(curl -fsSL -o /dev/null -w '%{url_effective}' -L "https://github.com/${REPO}/releases/latest" || true)
    tag=$(printf '%s' "$tag_url" | sed 's#.*/tag/##' | sed 's/^v//')

    [ -z "$tag" ] && err "Could not determine latest version. Set VERSION=x.y.z and retry. (latest URL: ${tag_url})"
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
    HTTP_CODE=$(curl -fSL -w '%{http_code}' -o "${TMPDIR_DL}/${ASSET}" "$URL" 2>/dev/null || printf '000')

    [ "$HTTP_CODE" != "200" ] && err "Download failed (HTTP ${HTTP_CODE}). Check that v${VER} exists at:\n         ${URL}"

    # Verify SHA256 checksum (published alongside the release asset)
    info "Verifying checksum…"
    SHA_URL="${URL}.sha256"
    HTTP_CODE_SHA=$(curl -fSL -w '%{http_code}' -o "${TMPDIR_DL}/${ASSET}.sha256" "$SHA_URL" 2>/dev/null || printf '000')
    [ "$HTTP_CODE_SHA" != "200" ] && err "Checksum download failed (HTTP ${HTTP_CODE_SHA}). Expected:\n         ${SHA_URL}"

    EXPECTED=$(cut -d ' ' -f 1 < "${TMPDIR_DL}/${ASSET}.sha256" | tr -d '\n')
    ACTUAL=$(sha256_file "${TMPDIR_DL}/${ASSET}" | tr -d '\n')

    [ -z "$EXPECTED" ] && err "Checksum file was empty or invalid: ${TMPDIR_DL}/${ASSET}.sha256"
    [ "$EXPECTED" != "$ACTUAL" ] && err "Checksum mismatch for ${ASSET}.\n         Expected: ${EXPECTED}\n         Actual:   ${ACTUAL}"
    ok "Checksum OK"

    # Extract (defensive: ensure the archive contains only the expected binary)
    info "Extracting…"
    ENTRIES=$(tar -tzf "${TMPDIR_DL}/${ASSET}" 2>/dev/null || true)
    COUNT=$(printf '%s\n' "$ENTRIES" | sed '/^$/d' | wc -l | tr -d ' ')
    ENTRY_RAW=$(printf '%s\n' "$ENTRIES" | head -n 1)
    ENTRY_NORM=$(printf '%s' "$ENTRY_RAW" | sed 's#^\./##')

    [ "$COUNT" != "1" ] && err "Archive contained unexpected files:\n$ENTRIES"
    [ "$ENTRY_NORM" != "$BINARY" ] && err "Archive did not contain expected '${BINARY}' binary (found: '${ENTRY_RAW}')."

    tar -xzf "${TMPDIR_DL}/${ASSET}" -C "$TMPDIR_DL" "$ENTRY_RAW"
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
