#!/bin/sh

set -eu

GRAYSLATE_PACKAGES_URL="https://packages.grayslate.app"
GRAYSLATE_RELEASES_URL="https://github.com/shriram-ethiraj/grayslate/releases/latest"
GRAYSLATE_APT_KEY_SHA256="@GRAYSLATE_APT_KEY_SHA256@"
GRAYSLATE_APT_CONFIG_SHA256="@GRAYSLATE_APT_CONFIG_SHA256@"
GRAYSLATE_RPM_KEY_SHA256="@GRAYSLATE_RPM_KEY_SHA256@"
GRAYSLATE_RPM_CONFIG_SHA256="@GRAYSLATE_RPM_CONFIG_SHA256@"
GRAYSLATE_TEMP_DIR=""
GRAYSLATE_ELEVATE=""

say() {
    printf '%s\n' "$*"
}

fail() {
    printf 'Grayslate installer: %s\n' "$*" >&2
    printf 'Standalone AppImage: %s\n' "$GRAYSLATE_RELEASES_URL" >&2
    exit 1
}

cleanup() {
    if [ -n "$GRAYSLATE_TEMP_DIR" ] && [ -d "$GRAYSLATE_TEMP_DIR" ]; then
        rm -rf "$GRAYSLATE_TEMP_DIR"
    fi
}

run_as_root() {
    if [ -n "$GRAYSLATE_ELEVATE" ]; then
        "$GRAYSLATE_ELEVATE" "$@"
    else
        "$@"
    fi
}

download() {
    source_url=$1
    destination=$2
    curl --proto '=https' --tlsv1.2 -fsSL "$source_url" -o "$destination"
    [ -s "$destination" ] || fail "Downloaded an empty file from $source_url."
}

verify_sha256() {
    expected_hash=$1
    path=$2
    actual_hash=$(sha256sum "$path" | awk '{ print $1 }')
    [ "$actual_hash" = "$expected_hash" ] || fail "Checksum verification failed for $(basename "$path")."
}

os_release_value() {
    key=$1
    sed -n "s/^${key}=//p" /etc/os-release | head -n 1 | tr -d '"' | tr '[:upper:]' '[:lower:]'
}

detect_package_family() {
    [ -r /etc/os-release ] || fail "Cannot identify this Linux distribution because /etc/os-release is missing."

    distro_id=$(os_release_value ID)
    distro_like=$(os_release_value ID_LIKE)
    distro_tokens=" $distro_id $distro_like "

    case "$distro_tokens" in
        *" debian "*) printf '%s\n' "apt" ;;
        *" fedora "*|*" rhel "*|*" centos "*) printf '%s\n' "dnf" ;;
        *) fail "Unsupported distribution '$distro_id'. Supported families are Debian/Ubuntu/Mint and Fedora/RHEL." ;;
    esac
}

prepare_privileges() {
    if [ "$(id -u)" -eq 0 ]; then
        GRAYSLATE_ELEVATE=""
    elif command -v sudo >/dev/null 2>&1; then
        GRAYSLATE_ELEVATE="sudo"
    else
        fail "Administrator access is required. Install sudo or run this installer as root."
    fi
}

install_with_apt() {
    command -v apt-get >/dev/null 2>&1 || fail "This distribution is Debian-based, but apt-get is unavailable."

    apt_key="$GRAYSLATE_TEMP_DIR/grayslate-archive-keyring.gpg"
    apt_sources="$GRAYSLATE_TEMP_DIR/grayslate.sources"
    download "$GRAYSLATE_PACKAGES_URL/keys/grayslate-archive-keyring.gpg" "$apt_key"
    download "$GRAYSLATE_PACKAGES_URL/config/grayslate.sources" "$apt_sources"
    verify_sha256 "$GRAYSLATE_APT_KEY_SHA256" "$apt_key"
    verify_sha256 "$GRAYSLATE_APT_CONFIG_SHA256" "$apt_sources"

    grep -Fqx "URIs: $GRAYSLATE_PACKAGES_URL/apt" "$apt_sources" || fail "The downloaded APT source has an unexpected repository URL."
    grep -Fqx "Signed-By: /etc/apt/keyrings/grayslate-archive-keyring.gpg" "$apt_sources" || fail "The downloaded APT source has an unexpected signing-key path."

    say "Configuring the signed Grayslate APT repository..."
    run_as_root install -d -m 0755 /etc/apt/keyrings /etc/apt/sources.list.d
    run_as_root install -m 0644 "$apt_key" /etc/apt/keyrings/grayslate-archive-keyring.gpg
    run_as_root install -m 0644 "$apt_sources" /etc/apt/sources.list.d/grayslate.sources
    run_as_root apt-get update
    run_as_root apt-get install -y grayslate
}

install_with_dnf() {
    command -v dnf >/dev/null 2>&1 || fail "This distribution is RPM-based, but dnf is unavailable."
    command -v rpm >/dev/null 2>&1 || fail "This distribution is RPM-based, but rpm is unavailable."

    rpm_key="$GRAYSLATE_TEMP_DIR/grayslate-archive-key.asc"
    rpm_repository="$GRAYSLATE_TEMP_DIR/grayslate.repo"
    download "$GRAYSLATE_PACKAGES_URL/keys/grayslate-archive-key.asc" "$rpm_key"
    download "$GRAYSLATE_PACKAGES_URL/config/grayslate.repo" "$rpm_repository"
    verify_sha256 "$GRAYSLATE_RPM_KEY_SHA256" "$rpm_key"
    verify_sha256 "$GRAYSLATE_RPM_CONFIG_SHA256" "$rpm_repository"

    grep -Fqx 'baseurl=https://packages.grayslate.app/rpm/stable/$basearch' "$rpm_repository" || fail "The downloaded DNF configuration has an unexpected repository URL."
    grep -Fqx "gpgkey=$GRAYSLATE_PACKAGES_URL/keys/grayslate-archive-key.asc" "$rpm_repository" || fail "The downloaded DNF configuration has an unexpected signing-key URL."
    grep -Fqx "gpgcheck=1" "$rpm_repository" || fail "The downloaded DNF configuration does not require package signatures."
    grep -Fqx "repo_gpgcheck=1" "$rpm_repository" || fail "The downloaded DNF configuration does not require repository signatures."

    say "Configuring the signed Grayslate DNF repository..."
    run_as_root install -d -m 0755 /etc/yum.repos.d
    run_as_root rpm --import "$rpm_key"
    run_as_root install -m 0644 "$rpm_repository" /etc/yum.repos.d/grayslate.repo
    run_as_root dnf install -y grayslate
}

main() {
    [ "$(uname -s)" = "Linux" ] || fail "This installer only supports Linux."
    case "$(uname -m)" in
        x86_64|amd64) ;;
        *) fail "Unsupported architecture '$(uname -m)'. Current Linux packages require x86_64." ;;
    esac
    command -v curl >/dev/null 2>&1 || fail "curl is required to download repository configuration."
    command -v install >/dev/null 2>&1 || fail "The standard install utility is required."
    command -v sha256sum >/dev/null 2>&1 || fail "sha256sum is required to verify repository files."

    package_family=$(detect_package_family)
    prepare_privileges
    GRAYSLATE_TEMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/grayslate-install.XXXXXX") || fail "Could not create a temporary directory."
    trap cleanup EXIT HUP INT TERM

    say "Installing Grayslate for the detected $package_family package system."
    case "$package_family" in
        apt) install_with_apt ;;
        dnf) install_with_dnf ;;
        *) fail "Internal error: unsupported package family '$package_family'." ;;
    esac
    say "Grayslate is installed. Future updates will arrive through your system package manager."
}

main "$@"
