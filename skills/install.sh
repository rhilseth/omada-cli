#!/usr/bin/env bash
# Install the omada skill into an agent harness's skill directory.
#
# Default behavior: symlink this repo's skills/omada directory into
# ~/.claude/skills/omada, so `git pull` in the repo updates the skill
# in-place with no re-install step.

set -euo pipefail

TARGET="claude-code"
MODE="symlink"
ACTION="install"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_SRC="$SCRIPT_DIR/omada"

usage() {
    cat <<EOF
Usage: $(basename "$0") [--target HARNESS] [--copy] [--uninstall] [--help]

Install the omada skill into an agent harness so Claude (or another
agent) automatically learns how to drive the omada CLI.

Options:
  --target HARNESS  Agent harness to install for. Default: claude-code.
                    Supported: claude-code
  --copy            Copy files instead of symlinking. Use this only if
                    your filesystem does not support symlinks; updates
                    to the repo won't propagate until you re-run install.
  --uninstall       Remove the installed skill.
  --help, -h        Show this help.

Examples:
  $(basename "$0")                  # symlink into ~/.claude/skills/omada
  $(basename "$0") --copy           # copy instead of symlink
  $(basename "$0") --uninstall      # remove the installation
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target) TARGET="${2:-}"; shift 2 ;;
        --copy) MODE="copy"; shift ;;
        --uninstall) ACTION="uninstall"; shift ;;
        --help|-h) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
    esac
done

case "$TARGET" in
    claude-code)
        DEST_DIR="$HOME/.claude/skills/omada"
        ;;
    *)
        echo "Unsupported target: $TARGET" >&2
        echo "Supported targets: claude-code" >&2
        exit 1
        ;;
esac

if [[ "$ACTION" == "uninstall" ]]; then
    if [[ -L "$DEST_DIR" || -e "$DEST_DIR" ]]; then
        rm -rf "$DEST_DIR"
        echo "Removed $DEST_DIR"
    else
        echo "Nothing to remove at $DEST_DIR"
    fi
    exit 0
fi

if [[ ! -f "$SKILL_SRC/SKILL.md" ]]; then
    echo "SKILL.md not found at $SKILL_SRC/SKILL.md" >&2
    exit 1
fi

mkdir -p "$(dirname "$DEST_DIR")"

if [[ -L "$DEST_DIR" || -e "$DEST_DIR" ]]; then
    echo "Replacing existing $DEST_DIR"
    rm -rf "$DEST_DIR"
fi

case "$MODE" in
    symlink)
        ln -s "$SKILL_SRC" "$DEST_DIR"
        echo "Symlinked $DEST_DIR -> $SKILL_SRC"
        ;;
    copy)
        mkdir -p "$DEST_DIR"
        cp "$SKILL_SRC/SKILL.md" "$DEST_DIR/SKILL.md"
        echo "Copied SKILL.md to $DEST_DIR"
        ;;
esac

echo "Installed omada skill for $TARGET."
