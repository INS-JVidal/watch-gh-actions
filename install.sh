#!/bin/sh
# Install script for ghw (GitHub Actions Watcher) and glw (GitLab CI Watcher)
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/INS-JVidal/watch-gh-actions/master/install.sh | sh
#   curl -sSL ... | sh -s -- ghw       # install only ghw
#   curl -sSL ... | sh -s -- glw       # install only glw

set -eu

REPO="INS-JVidal/watch-gh-actions"
INSTALL_DIR="${HOME}/.local/bin"

# --- Colors -----------------------------------------------------------

if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    CYAN='\033[0;36m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    RED='' GREEN='' YELLOW='' CYAN='' BOLD='' RESET=''
fi

info()  { printf "${CYAN}info${RESET}  %s\n" "$*"; }
ok()    { printf "${GREEN}  ok${RESET}  %s\n" "$*"; }
warn()  { printf "${YELLOW}warn${RESET}  %s\n" "$*"; }
error() { printf "${RED}error${RESET} %s\n" "$*" >&2; }
die()   { error "$@"; exit 1; }

# --- Parse arguments ---------------------------------------------------

BINARIES=""
case "${1:-both}" in
    ghw)  BINARIES="ghw" ;;
    glw)  BINARIES="glw" ;;
    both) BINARIES="ghw glw" ;;
    -h|--help)
        printf "Usage: install.sh [ghw|glw]\n"
        printf "  ghw   Install GitHub Actions Watcher only\n"
        printf "  glw   Install GitLab CI Watcher only\n"
        printf "  (no arg) Install both\n"
        exit 0
        ;;
    *)
        die "Unknown argument: $1 (expected 'ghw', 'glw', or no argument)"
        ;;
esac

# --- Detect platform ---------------------------------------------------

OS="$(uname -s)"
ARCH="$(uname -m)"

info "Detected platform: ${OS} ${ARCH}"

# --- Install function for precompiled binaries -------------------------

install_precompiled() {
    info "Fetching latest release from GitHub..."

    if ! command -v curl >/dev/null 2>&1; then
        die "curl is required but not found"
    fi

    TAG="$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | head -1 | cut -d'"' -f4)"

    if [ -z "${TAG}" ]; then
        die "Could not determine latest release tag. Check your internet connection or try again later."
    fi

    info "Latest release: ${TAG}"

    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "${TMPDIR}"' EXIT

    mkdir -p "${INSTALL_DIR}"

    for bin in ${BINARIES}; do
        ARCHIVE="${bin}-${TAG}-x86_64-unknown-linux-gnu.tar.gz"
        URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"

        info "Downloading ${bin}..."
        if ! curl -fsSL -o "${TMPDIR}/${ARCHIVE}" "${URL}"; then
            die "Failed to download ${bin} from ${URL} (asset may not exist in this release)"
        fi

        tar -xzf "${TMPDIR}/${ARCHIVE}" -C "${TMPDIR}"

        if [ ! -f "${TMPDIR}/${bin}" ]; then
            die "Expected binary '${bin}' not found in archive"
        fi

        rm -f "${INSTALL_DIR}/${bin}"
        install -m 755 "${TMPDIR}/${bin}" "${INSTALL_DIR}/${bin}"
        ok "Installed ${bin} to ${INSTALL_DIR}/${bin}"
        rm -f "${TMPDIR}/${bin}" "${TMPDIR}/${ARCHIVE}"
    done
}

# --- Install function for building from source -------------------------

install_from_source() {
    warn "No precompiled binary available for ${OS} ${ARCH}"
    info "Building from source..."

    if ! command -v cargo >/dev/null 2>&1; then
        error "Rust toolchain (cargo) is required to build from source."
        error "Install Rust via: https://rustup.rs"
        die "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    fi

    if ! command -v git >/dev/null 2>&1; then
        die "git is required to clone the repository"
    fi

    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "${TMPDIR}"' EXIT

    info "Cloning ${REPO}..."
    git clone --depth 1 "https://github.com/${REPO}.git" "${TMPDIR}/repo"

    for bin in ${BINARIES}; do
        info "Building ${bin} (this may take a few minutes)..."
        cargo install --path "${TMPDIR}/repo/crates/${bin}" --root "${HOME}/.local"
        ok "Installed ${bin} to ${INSTALL_DIR}/${bin}"
    done
}

# --- Main --------------------------------------------------------------

case "${OS}-${ARCH}" in
    Linux-x86_64)
        install_precompiled
        ;;
    *)
        install_from_source
        ;;
esac

# --- PATH check --------------------------------------------------------

case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
        ;;
    *)
        echo ""
        warn "${INSTALL_DIR} is not in your PATH."
        warn "Add it by appending this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        printf "  ${BOLD}export PATH=\"%s:\$PATH\"${RESET}\n" "${INSTALL_DIR}"
        echo ""
        ;;
esac

echo ""
ok "Done! Run 'ghw --help' or 'glw --help' to get started."
