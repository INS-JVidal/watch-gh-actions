#!/bin/bash
#
# Claude Code Hooks Monitor - Setup Script (Ubuntu Only)
# Installs all required system tools for Ubuntu/Debian systems
# Safe to run multiple times - checks before installing
# Version: 2.0 - Production Ready
#

# DO NOT use 'set -e' - we want to handle errors gracefully
set -u  # Exit on undefined variable
set -o pipefail  # Catch errors in pipes

# Colors for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly CYAN='\033[0;36m'
readonly NC='\033[0m' # No Color

# Symbols
readonly CHECK="${GREEN}✓${NC}"
readonly CROSS="${RED}✗${NC}"
readonly ARROW="${BLUE}→${NC}"
readonly WARN="${YELLOW}⚠${NC}"

# Configuration
readonly MIN_GO_VERSION="1.21"
readonly MIN_PYTHON_VERSION="3.11"
readonly TEMP_DIR="/tmp/claude-hooks-setup-$$"
readonly GO_INSTALL_DIR="/usr/local/go"

# Track what was installed
declare -a INSTALLED_ITEMS=()
declare -a SKIPPED_ITEMS=()
declare -a FAILED_ITEMS=()

# Cleanup on exit
cleanup() {
    if [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}
trap cleanup EXIT

# Create temp directory
mkdir -p "$TEMP_DIR"

# Print banner
print_banner() {
    echo -e "${CYAN}"
    echo "╔════════════════════════════════════════════════════════════════╗"
    echo "║     Claude Code Hooks Monitor - Setup Script (Ubuntu)         ║"
    echo "║     Installing required system tools...                       ║"
    echo "╚════════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# Check if running on Ubuntu/Debian
check_ubuntu() {
    if [ ! -f /etc/os-release ]; then
        echo -e "${CROSS} Cannot detect OS. This script is for Ubuntu/Debian only."
        return 1
    fi
    
    # Source the file safely
    local os_id=""
    local os_id_like=""
    local os_pretty_name=""
    
    # Parse /etc/os-release
    while IFS='=' read -r key value; do
        # Remove quotes from value
        value="${value%\"}"
        value="${value#\"}"
        
        case "$key" in
            ID) os_id="$value" ;;
            ID_LIKE) os_id_like="$value" ;;
            PRETTY_NAME) os_pretty_name="$value" ;;
        esac
    done < /etc/os-release
    
    if [[ "$os_id" != "ubuntu" ]] && [[ "$os_id" != "debian" ]] && \
       [[ "$os_id_like" != *"ubuntu"* ]] && [[ "$os_id_like" != *"debian"* ]]; then
        echo -e "${CROSS} This script is designed for Ubuntu/Debian systems only."
        echo -e "${YELLOW}  Detected: ${os_pretty_name}${NC}"
        return 1
    fi
    
    echo -e "${ARROW} Detected: ${os_pretty_name}"
    echo ""
    return 0
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Compare two version numbers
# Returns 0 if version1 >= version2
version_ge() {
    local version1="$1"
    local version2="$2"
    
    # Handle empty versions
    [ -z "$version1" ] && return 1
    [ -z "$version2" ] && return 0
    
    # Use sort -V for version comparison
    local sorted_first=$(printf '%s\n%s' "$version1" "$version2" | sort -V | head -n1)
    
    [ "$sorted_first" = "$version2" ]
}

# Extract version from command output
extract_version() {
    local output="$1"
    echo "$output" | grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n 1
}

# Check version
check_version() {
    local cmd=$1
    local min_version=$2
    local version_flag=${3:---version}
    
    if ! command_exists "$cmd"; then
        return 1
    fi
    
    local version_output
    if ! version_output=$($cmd $version_flag 2>&1); then
        return 1
    fi
    
    local current_version
    current_version=$(extract_version "$version_output")
    
    if [ -z "$current_version" ]; then
        return 1
    fi
    
    version_ge "$current_version" "$min_version"
}

# Add to PATH in shell config if not already present
add_to_path() {
    local path_to_add="$1"
    local shell_config="$2"
    local export_line="$3"
    
    # Check if file exists
    if [ ! -f "$shell_config" ]; then
        touch "$shell_config"
    fi
    
    # Check if already in config (exact match to avoid duplicates)
    if grep -Fxq "$export_line" "$shell_config" 2>/dev/null; then
        return 0  # Already exists
    fi
    
    # Also check for similar entries
    if grep -q "$path_to_add" "$shell_config" 2>/dev/null; then
        return 0  # Similar entry exists
    fi
    
    # Add to config
    echo "$export_line" >> "$shell_config"
    return 0
}

# Get latest stable Go version
get_latest_go_version() {
    local latest_version
    
    # Try to get from golang.org
    if command_exists curl; then
        latest_version=$(curl -sL https://go.dev/VERSION?m=text 2>/dev/null | head -n1 | sed 's/go//')
    elif command_exists wget; then
        latest_version=$(wget -qO- https://go.dev/VERSION?m=text 2>/dev/null | head -n1 | sed 's/go//')
    fi
    
    # Validate version format
    if [[ "$latest_version" =~ ^[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
        echo "$latest_version"
    else
        # Fallback to a known good version
        echo "1.21.6"
    fi
}

# Update apt cache
update_apt_cache() {
    echo -e "${ARROW} Updating package lists..."
    
    if ! sudo apt-get update >/dev/null 2>&1; then
        echo -e "${WARN} Failed to update package lists"
        echo -e "${YELLOW}  This might cause installation issues${NC}"
        return 1
    fi
    
    echo -e "${CHECK} Package lists updated"
    echo ""
    return 0
}

# Install curl
install_curl() {
    echo -e "${ARROW} Checking for curl..."
    
    if command_exists curl; then
        echo -e "${CHECK} curl already installed"
        SKIPPED_ITEMS+=("curl")
        echo ""
        return 0
    fi
    
    echo -e "${YELLOW}→${NC} Installing curl..."
    
    if ! sudo apt-get install -y curl >/dev/null 2>&1; then
        echo -e "${CROSS} Failed to install curl"
        FAILED_ITEMS+=("curl")
        echo ""
        return 1
    fi
    
    echo -e "${CHECK} curl installed successfully"
    INSTALLED_ITEMS+=("curl")
    echo ""
    return 0
}

# Install wget
install_wget() {
    echo -e "${ARROW} Checking for wget..."
    
    if command_exists wget; then
        echo -e "${CHECK} wget already installed"
        SKIPPED_ITEMS+=("wget")
        echo ""
        return 0
    fi
    
    echo -e "${YELLOW}→${NC} Installing wget..."
    
    if ! sudo apt-get install -y wget >/dev/null 2>&1; then
        echo -e "${CROSS} Failed to install wget"
        FAILED_ITEMS+=("wget")
        echo ""
        return 1
    fi
    
    echo -e "${CHECK} wget installed successfully"
    INSTALLED_ITEMS+=("wget")
    echo ""
    return 0
}

# Install Go
install_go() {
    echo -e "${ARROW} Checking for Go (>= ${MIN_GO_VERSION})..."
    
    if check_version go "$MIN_GO_VERSION" version; then
        local go_version
        go_version=$(go version 2>&1 | grep -oE 'go[0-9]+\.[0-9]+(\.[0-9]+)?' | sed 's/go//')
        echo -e "${CHECK} Go already installed (version ${go_version})"
        SKIPPED_ITEMS+=("Go")
        echo ""
        return 0
    fi
    
    echo -e "${YELLOW}→${NC} Installing latest stable Go version..."
    echo -e "${BLUE}  Getting latest Go version...${NC}"
    
    # Get latest version
    local go_version
    go_version=$(get_latest_go_version)
    
    if [ -z "$go_version" ]; then
        echo -e "${CROSS} Failed to determine Go version"
        FAILED_ITEMS+=("Go")
        echo ""
        return 1
    fi
    
    echo -e "${BLUE}  Latest Go version: ${go_version}${NC}"
    
    # Determine architecture
    local go_arch="amd64"
    local machine_arch
    machine_arch=$(uname -m)
    
    case "$machine_arch" in
        x86_64)
            go_arch="amd64"
            ;;
        aarch64|arm64)
            go_arch="arm64"
            ;;
        armv7l|armv6l)
            go_arch="armv6l"
            ;;
        *)
            echo -e "${CROSS} Unsupported architecture: ${machine_arch}"
            FAILED_ITEMS+=("Go")
            echo ""
            return 1
            ;;
    esac
    
    local go_tarball="go${go_version}.linux-${go_arch}.tar.gz"
    local go_url="https://go.dev/dl/${go_tarball}"
    local go_tarball_path="${TEMP_DIR}/${go_tarball}"
    
    # Download Go
    echo -e "${BLUE}  Downloading Go ${go_version} for ${go_arch}...${NC}"
    
    if command_exists wget; then
        if ! wget -q --show-progress "$go_url" -O "$go_tarball_path" 2>&1; then
            echo -e "${CROSS} Failed to download Go"
            FAILED_ITEMS+=("Go")
            echo ""
            return 1
        fi
    elif command_exists curl; then
        if ! curl -# -L "$go_url" -o "$go_tarball_path" 2>&1; then
            echo -e "${CROSS} Failed to download Go"
            FAILED_ITEMS+=("Go")
            echo ""
            return 1
        fi
    else
        echo -e "${CROSS} Neither wget nor curl available"
        FAILED_ITEMS+=("Go")
        echo ""
        return 1
    fi
    
    # Verify download
    if [ ! -f "$go_tarball_path" ] || [ ! -s "$go_tarball_path" ]; then
        echo -e "${CROSS} Downloaded file is empty or missing"
        FAILED_ITEMS+=("Go")
        echo ""
        return 1
    fi
    
    # Remove old Go installation
    echo -e "${BLUE}  Removing old Go installation (if exists)...${NC}"
    if ! sudo rm -rf "$GO_INSTALL_DIR"; then
        echo -e "${WARN} Failed to remove old Go installation"
    fi
    
    # Extract Go
    echo -e "${BLUE}  Extracting to ${GO_INSTALL_DIR}...${NC}"
    if ! sudo tar -C /usr/local -xzf "$go_tarball_path"; then
        echo -e "${CROSS} Failed to extract Go"
        FAILED_ITEMS+=("Go")
        echo ""
        return 1
    fi
    
    # Add to PATH
    local go_path_export='export PATH=$PATH:/usr/local/go/bin'
    
    if [ -f "$HOME/.bashrc" ]; then
        if add_to_path "/usr/local/go/bin" "$HOME/.bashrc" "$go_path_export"; then
            echo -e "${BLUE}  Added Go to ~/.bashrc${NC}"
        fi
    fi
    
    if [ -f "$HOME/.zshrc" ]; then
        if add_to_path "/usr/local/go/bin" "$HOME/.zshrc" "$go_path_export"; then
            echo -e "${BLUE}  Added Go to ~/.zshrc${NC}"
        fi
    fi
    
    # Add to current session
    export PATH=$PATH:/usr/local/go/bin
    
    # Verify installation
    if check_version go "$MIN_GO_VERSION" version; then
        local installed_version
        installed_version=$(go version 2>&1 | grep -oE 'go[0-9]+\.[0-9]+(\.[0-9]+)?' | sed 's/go//')
        echo -e "${CHECK} Go installed successfully (version ${installed_version})"
        echo -e "${BLUE}  Note: Restart your terminal or run 'source ~/.bashrc' to use Go${NC}"
        INSTALLED_ITEMS+=("Go")
        echo ""
        return 0
    else
        echo -e "${CROSS} Go installation verification failed"
        FAILED_ITEMS+=("Go")
        echo ""
        return 1
    fi
}

# Install Python 3.11+
install_python() {
    echo -e "${ARROW} Checking for Python (>= ${MIN_PYTHON_VERSION})..."
    
    # Check if python3 meets requirements
    if command_exists python3; then
        local py_version
        py_version=$(python3 --version 2>&1 | grep -oE '[0-9]+\.[0-9]+' | head -n 1)
        
        if version_ge "$py_version" "$MIN_PYTHON_VERSION"; then
            echo -e "${CHECK} Python already installed (version ${py_version})"
            SKIPPED_ITEMS+=("Python")
            echo ""
            return 0
        else
            echo -e "${WARN} Python ${py_version} found, but need >= ${MIN_PYTHON_VERSION}"
        fi
    fi
    
    echo -e "${YELLOW}→${NC} Installing Python ${MIN_PYTHON_VERSION}..."
    
    # Check if add-apt-repository is available
    if ! command_exists add-apt-repository; then
        echo -e "${BLUE}  Installing software-properties-common...${NC}"
        if ! sudo apt-get install -y software-properties-common >/dev/null 2>&1; then
            echo -e "${CROSS} Failed to install software-properties-common"
            FAILED_ITEMS+=("Python")
            echo ""
            return 1
        fi
    fi
    
    # Add deadsnakes PPA
    echo -e "${BLUE}  Adding deadsnakes PPA...${NC}"
    if ! sudo add-apt-repository -y ppa:deadsnakes/ppa >/dev/null 2>&1; then
        echo -e "${CROSS} Failed to add deadsnakes PPA"
        FAILED_ITEMS+=("Python")
        echo ""
        return 1
    fi
    
    # Update apt cache
    if ! sudo apt-get update >/dev/null 2>&1; then
        echo -e "${WARN} Failed to update package lists after adding PPA"
    fi
    
    # Install Python 3.11
    echo -e "${BLUE}  Installing Python 3.11 packages...${NC}"
    if ! sudo apt-get install -y python3.11 python3.11-venv python3.11-dev >/dev/null 2>&1; then
        echo -e "${CROSS} Failed to install Python 3.11"
        FAILED_ITEMS+=("Python")
        echo ""
        return 1
    fi
    
    # Set python3.11 as the default python3 (if update-alternatives is available)
    if command_exists update-alternatives; then
        sudo update-alternatives --install /usr/bin/python3 python3 /usr/bin/python3.11 1 >/dev/null 2>&1 || true
    fi
    
    # Verify installation
    if command_exists python3; then
        local py_version
        py_version=$(python3 --version 2>&1 | grep -oE '[0-9]+\.[0-9]+')
        echo -e "${CHECK} Python installed successfully (version ${py_version})"
        INSTALLED_ITEMS+=("Python")
        echo ""
        return 0
    else
        echo -e "${CROSS} Python installation verification failed"
        FAILED_ITEMS+=("Python")
        echo ""
        return 1
    fi
}

# Install uv
install_uv() {
    echo -e "${ARROW} Checking for uv..."
    
    if command_exists uv; then
        local uv_version
        uv_version=$(uv --version 2>&1 | head -n 1)
        echo -e "${CHECK} uv already installed (${uv_version})"
        SKIPPED_ITEMS+=("uv")
        echo ""
        return 0
    fi
    
    echo -e "${YELLOW}→${NC} Installing uv..."
    
    # Download and run installer
    if command_exists curl; then
        if ! curl -LsSf https://astral.sh/uv/install.sh | sh >/dev/null 2>&1; then
            echo -e "${CROSS} Failed to install uv"
            FAILED_ITEMS+=("uv")
            echo ""
            return 1
        fi
    else
        echo -e "${CROSS} curl not available for uv installation"
        FAILED_ITEMS+=("uv")
        echo ""
        return 1
    fi
    
    # Add to PATH
    local cargo_path_export='export PATH="$HOME/.cargo/bin:$PATH"'
    
    if [ -f "$HOME/.bashrc" ]; then
        if add_to_path "$HOME/.cargo/bin" "$HOME/.bashrc" "$cargo_path_export"; then
            echo -e "${BLUE}  Added uv to ~/.bashrc${NC}"
        fi
    fi
    
    if [ -f "$HOME/.zshrc" ]; then
        if add_to_path "$HOME/.cargo/bin" "$HOME/.zshrc" "$cargo_path_export"; then
            echo -e "${BLUE}  Added uv to ~/.zshrc${NC}"
        fi
    fi
    
    # Add to current session
    export PATH="$HOME/.cargo/bin:$PATH"
    
    # Verify installation
    if command_exists uv; then
        local uv_version
        uv_version=$(uv --version 2>&1 | head -n 1)
        echo -e "${CHECK} uv installed successfully (${uv_version})"
        echo -e "${BLUE}  Note: Restart your terminal or run 'source ~/.bashrc' to use uv${NC}"
        INSTALLED_ITEMS+=("uv")
        echo ""
        return 0
    else
        echo -e "${CROSS} uv installation verification failed"
        FAILED_ITEMS+=("uv")
        echo ""
        return 1
    fi
}

# Install jq
install_jq() {
    echo -e "${ARROW} Checking for jq..."
    
    if command_exists jq; then
        local jq_version
        jq_version=$(jq --version 2>&1)
        echo -e "${CHECK} jq already installed (${jq_version})"
        SKIPPED_ITEMS+=("jq")
        echo ""
        return 0
    fi
    
    echo -e "${YELLOW}→${NC} Installing jq..."
    
    if ! sudo apt-get install -y jq >/dev/null 2>&1; then
        echo -e "${CROSS} Failed to install jq"
        FAILED_ITEMS+=("jq")
        echo ""
        return 1
    fi
    
    if command_exists jq; then
        local jq_version
        jq_version=$(jq --version 2>&1)
        echo -e "${CHECK} jq installed successfully (${jq_version})"
        INSTALLED_ITEMS+=("jq")
    else
        echo -e "${CROSS} jq installation verification failed"
        FAILED_ITEMS+=("jq")
    fi
    
    echo ""
    return 0
}

# Install git
install_git() {
    echo -e "${ARROW} Checking for git..."
    
    if command_exists git; then
        local git_version
        git_version=$(git --version 2>&1)
        echo -e "${CHECK} git already installed (${git_version})"
        SKIPPED_ITEMS+=("git")
        echo ""
        return 0
    fi
    
    echo -e "${YELLOW}→${NC} Installing git..."
    
    if ! sudo apt-get install -y git >/dev/null 2>&1; then
        echo -e "${CROSS} Failed to install git"
        FAILED_ITEMS+=("git")
        echo ""
        return 1
    fi
    
    if command_exists git; then
        local git_version
        git_version=$(git --version 2>&1)
        echo -e "${CHECK} git installed successfully (${git_version})"
        INSTALLED_ITEMS+=("git")
    else
        echo -e "${CROSS} git installation verification failed"
        FAILED_ITEMS+=("git")
    fi
    
    echo ""
    return 0
}

# Install make
install_make() {
    echo -e "${ARROW} Checking for make..."
    
    if command_exists make; then
        local make_version
        make_version=$(make --version 2>&1 | head -n 1)
        echo -e "${CHECK} make already installed (${make_version})"
        SKIPPED_ITEMS+=("make")
        echo ""
        return 0
    fi
    
    echo -e "${YELLOW}→${NC} Installing build-essential (includes make)..."
    
    if ! sudo apt-get install -y build-essential >/dev/null 2>&1; then
        echo -e "${CROSS} Failed to install build-essential"
        FAILED_ITEMS+=("build-essential")
        echo ""
        return 1
    fi
    
    if command_exists make; then
        echo -e "${CHECK} build-essential installed successfully"
        INSTALLED_ITEMS+=("build-essential")
    else
        echo -e "${CROSS} build-essential installation verification failed"
        FAILED_ITEMS+=("build-essential")
    fi
    
    echo ""
    return 0
}

# Verify all installations
verify_installations() {
    echo ""
    echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}Verification Summary${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
    
    local all_ok=true
    
    # Required tools with version checks
    if check_version go "$MIN_GO_VERSION" version; then
        local go_version
        go_version=$(go version 2>&1 | grep -oE 'go[0-9]+\.[0-9]+(\.[0-9]+)?' | sed 's/go//')
        echo -e "${CHECK} Go ${go_version} - OK"
    else
        echo -e "${CROSS} Go - NOT FOUND or version < ${MIN_GO_VERSION}"
        all_ok=false
    fi
    
    if command_exists python3; then
        local py_version
        py_version=$(python3 --version 2>&1 | grep -oE '[0-9]+\.[0-9]+')
        if version_ge "$py_version" "$MIN_PYTHON_VERSION"; then
            echo -e "${CHECK} Python ${py_version} - OK"
        else
            echo -e "${CROSS} Python ${py_version} - Version < ${MIN_PYTHON_VERSION}"
            all_ok=false
        fi
    else
        echo -e "${CROSS} Python - NOT FOUND"
        all_ok=false
    fi
    
    if command_exists uv; then
        local uv_version
        uv_version=$(uv --version 2>&1 | head -n 1)
        echo -e "${CHECK} ${uv_version} - OK"
    else
        echo -e "${CROSS} uv - NOT FOUND"
        all_ok=false
    fi
    
    if command_exists curl; then
        echo -e "${CHECK} curl - OK"
    else
        echo -e "${CROSS} curl - NOT FOUND"
        all_ok=false
    fi
    
    if command_exists wget; then
        echo -e "${CHECK} wget - OK"
    else
        echo -e "${CROSS} wget - NOT FOUND"
        all_ok=false
    fi
    
    if command_exists jq; then
        echo -e "${CHECK} jq - OK"
    else
        echo -e "${CROSS} jq - NOT FOUND"
        all_ok=false
    fi
    
    if command_exists git; then
        echo -e "${CHECK} git - OK"
    else
        echo -e "${CROSS} git - NOT FOUND"
        all_ok=false
    fi
    
    if command_exists make; then
        echo -e "${CHECK} make - OK"
    else
        echo -e "${CROSS} make - NOT FOUND"
        all_ok=false
    fi
    
    echo ""
    
    if $all_ok; then
        echo -e "${GREEN}✓ All required tools are installed!${NC}"
        return 0
    else
        echo -e "${RED}✗ Some tools are missing or outdated${NC}"
        return 1
    fi
}

# Print summary
print_summary() {
    echo ""
    echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}Installation Summary${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
    
    if [ ${#INSTALLED_ITEMS[@]} -gt 0 ]; then
        echo -e "${GREEN}Newly Installed (${#INSTALLED_ITEMS[@]}):${NC}"
        for item in "${INSTALLED_ITEMS[@]}"; do
            echo -e "  ${CHECK} $item"
        done
        echo ""
    fi
    
    if [ ${#SKIPPED_ITEMS[@]} -gt 0 ]; then
        echo -e "${BLUE}Already Installed (${#SKIPPED_ITEMS[@]}):${NC}"
        for item in "${SKIPPED_ITEMS[@]}"; do
            echo -e "  ${CHECK} $item"
        done
        echo ""
    fi
    
    if [ ${#FAILED_ITEMS[@]} -gt 0 ]; then
        echo -e "${RED}Failed to Install (${#FAILED_ITEMS[@]}):${NC}"
        for item in "${FAILED_ITEMS[@]}"; do
            echo -e "  ${CROSS} $item"
        done
        echo ""
        echo -e "${YELLOW}Please install failed items manually${NC}"
        echo ""
    fi
}

# Print next steps
print_next_steps() {
    echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}Next Steps${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
    
    if [ ${#INSTALLED_ITEMS[@]} -gt 0 ]; then
        echo -e "${YELLOW}→${NC} Restart your terminal or run:"
        echo -e "  ${BLUE}source ~/.bashrc${NC}"
        echo ""
    fi
    
    echo -e "${YELLOW}→${NC} Build and run the project:"
    echo -e "  ${BLUE}cd claude-hooks-monitor${NC}"
    echo -e "  ${BLUE}make deps${NC}           # Install Go dependencies"
    echo -e "  ${BLUE}make run${NC}            # Start the monitor"
    echo ""
    
    echo -e "${YELLOW}→${NC} Test without Claude Code:"
    echo -e "  ${BLUE}./test-hooks.sh${NC}"
    echo ""
    
    echo -e "${YELLOW}→${NC} Read the documentation:"
    echo -e "  ${BLUE}cat README.md${NC}"
    echo -e "  ${BLUE}cat QUICKSTART.md${NC}"
    echo ""
    
    echo -e "${GREEN}Happy monitoring! 🎣${NC}"
}

# Main execution
main() {
    print_banner
    
    # Check OS
    if ! check_ubuntu; then
        exit 1
    fi
    
    # Update apt cache first
    update_apt_cache || true  # Continue even if update fails
    
    # Install essential tools first (needed by other installations)
    install_curl || true
    install_wget || true
    
    # Install main tools (continue on failure)
    install_go || true
    install_python || true
    install_uv || true
    install_jq || true
    install_git || true
    install_make || true
    
    # Verify everything and print results
    local verification_result=0
    verify_installations || verification_result=$?
    
    print_summary
    
    if [ $verification_result -eq 0 ] && [ ${#FAILED_ITEMS[@]} -eq 0 ]; then
        print_next_steps
        exit 0
    else
        echo -e "${YELLOW}Please install missing/failed tools manually and run this script again${NC}"
        exit 1
    fi
}

# Check if running with sudo (not recommended)
if [ "$EUID" -eq 0 ]; then
    echo -e "${WARN} This script should not be run as root/sudo"
    echo -e "${YELLOW}  Please run as a normal user. The script will ask for sudo when needed.${NC}"
    exit 1
fi

# Check for internet connectivity
if ! command_exists ping || ! ping -c 1 8.8.8.8 >/dev/null 2>&1; then
    if ! command_exists curl || ! curl -s --head --connect-timeout 5 https://google.com >/dev/null 2>&1; then
        echo -e "${WARN} No internet connection detected"
        echo -e "${YELLOW}  This script requires internet access to download packages${NC}"
        echo -e "${YELLOW}  Please check your connection and try again${NC}"
        exit 1
    fi
fi

# Run main
main "$@"
