#!/bin/bash
# compress_video.sh - "Compress Video": one menu item, one dialog, two scenarios.
#
# The dialog has two tabs:
#   Presets     - fixed quality (compression level + codec)        -> compress_mp4.sh
#   Size limit  - fit the result into a file size limit            -> compress_limit.sh
# The fields of every tab are described by the script that encodes (compress_mp4.sh /
# compress_limit.sh print them with --print-tab), so the texts and the codec lists
# exist in one place only.
#
# Usage: compress_video.sh file1 [file2 ...]
#
# The script runs in two phases:
#   1. (no terminal) show the tabbed dialog (mmt-dialog)
#   2. re-launch the chosen script (MMT_PHASE=run) via mmt-terminal and encode

SELF="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(dirname "$SELF")"

# Detect user language, same logic as the installer (ru/uk/en)
_lang="${LANG:-${LC_MESSAGES:-en}}"
case "${_lang%%_*}" in
    ru|be|kk) MSG_TITLE="Сжать видео" ;;
    uk)       MSG_TITLE="Стиснути відео" ;;
    *)        MSG_TITLE="Compress Video" ;;
esac

[ $# -gt 0 ] || exit 0

# The form is JSON for mmt-dialog: the two tabs come from the encoding scripts.
build_form() {
    cat << EOF
{
  "title": "$MSG_TITLE",
  "tabs": [
$("$SCRIPT_DIR/compress_mp4.sh" --print-tab),
$("$SCRIPT_DIR/compress_limit.sh" --print-tab)
  ]
}
EOF
}

# Exit status 1 = cancelled, 2 = error (mmt-dialog has already shown it)
# --probe makes the dialog show resolution / bitrate of the selected files
ANSWERS=$(build_form | "$SCRIPT_DIR/mmt-dialog" --probe "$@") || exit 0

# answer <name> - value of one line "name=value" of the dialog output
answer() { sed -n "s/^$1=//p" <<< "$ANSWERS"; }

case "$(answer mode)" in
    preset)
        exec "$SCRIPT_DIR/mmt-terminal" env MMT_PHASE=run \
            MMT_LEVEL="$(answer level)" MMT_CODEC="$(answer codec)" \
            MMT_SCALE="$(answer scale)" \
            "$SCRIPT_DIR/compress_mp4.sh" "$@"
        ;;
    limit)
        exec "$SCRIPT_DIR/mmt-terminal" env MMT_PHASE=run \
            MMT_BUDGET="$(answer limit)" MMT_CODEC="$(answer codec)" \
            MMT_TWOPASS="$(answer twopass)" MMT_SCALE="$(answer scale)" \
            "$SCRIPT_DIR/compress_limit.sh" "$@"
        ;;
    *)
        echo "compress_video.sh: unknown mode in the dialog answer" >&2
        exit 1
        ;;
esac
