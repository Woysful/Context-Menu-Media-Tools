#!/bin/bash
# convert_webp_anim.sh - Convert video(s) to looping animated WebP (no audio).
#
# Asks for size limit, FPS, quality and compression level in a single dialog,
# then runs ffmpeg in a terminal window.
#
# Usage: convert_webp_anim.sh file1 [file2 ...]
#
# The script runs in two phases:
#   1. (no terminal) show the dialog (mmt-dialog)
#   2. (MMT_PHASE=run) re-launch itself via mmt-terminal and encode

SELF="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(dirname "$SELF")"

# Defaults
DEFAULT_SIZE=720
DEFAULT_FPS=20
DEFAULT_QUALITY=80
DEFAULT_COMPRESSION=5

# Detect user language, same logic as the installer (ru/uk/en)
_lang="${LANG:-${LC_MESSAGES:-en}}"
case "${_lang%%_*}" in
    ru|be|kk)
        MSG_TITLE="Конвертировать в анимированный WebP"
        MSG_SIZE="Лимит размера:"
        MSG_SIZE_HINT="Видео будет уменьшено так, чтобы его меньшая сторона не превышала это значение. Пропорции сохраняются, увеличения нет."
        MSG_FPS="FPS (кадров в секунду):"
        MSG_QUALITY="Качество (0-100):"
        MSG_COMPRESSION="Уровень сжатия (0-6):"
        MSG_COMPRESSION_HINT="Больше = меньше файл, но дольше."
        MSG_HEADER="Конвертация в WebP:"
        MSG_FAILED="Не удалось обработать:"
        MSG_PRESS_ENTER="Нажмите Enter, чтобы закрыть окно..."
        ;;
    uk)
        MSG_TITLE="Конвертувати в анімований WebP"
        MSG_SIZE="Ліміт розміру:"
        MSG_SIZE_HINT="Відео буде зменшено так, щоб його менша сторона не перевищувала це значення. Пропорції зберігаються, збільшення немає."
        MSG_FPS="FPS (кадрів за секунду):"
        MSG_QUALITY="Якість (0-100):"
        MSG_COMPRESSION="Рівень стиснення (0-6):"
        MSG_COMPRESSION_HINT="Більше = менший файл, але довше."
        MSG_HEADER="Конвертація у WebP:"
        MSG_FAILED="Не вдалося обробити:"
        MSG_PRESS_ENTER="Натисніть Enter, щоб закрити вікно..."
        ;;
    *)
        MSG_TITLE="Convert to WebP anim"
        MSG_SIZE="Size limit:"
        MSG_SIZE_HINT="The video is scaled down so its shorter side does not exceed this value. Aspect ratio is kept, no upscaling."
        MSG_FPS="FPS (frames per second):"
        MSG_QUALITY="Quality (0-100):"
        MSG_COMPRESSION="Compression level (0-6):"
        MSG_COMPRESSION_HINT="Higher = smaller file but slower."
        MSG_HEADER="Convert to WebP anim:"
        MSG_FAILED="Failed to process:"
        MSG_PRESS_ENTER="Press Enter to close this window..."
        ;;
esac

# The form is described as JSON for mmt-dialog.
# Keep the MSG_* texts free of double quotes and backslashes (they go into JSON).
# The ranges are enforced by the dialog itself, so no extra validation is needed.
build_form() {
    cat << EOF
{
  "title": "$MSG_TITLE",
  "fields": [
    {"id": "size", "type": "int", "label": "$MSG_SIZE", "default": $DEFAULT_SIZE,
     "min": 16, "max": 16384, "suffix": " px", "hint": "$MSG_SIZE_HINT"},
    {"id": "fps", "type": "int", "label": "$MSG_FPS", "default": $DEFAULT_FPS,
     "min": 1, "max": 120},
    {"id": "quality", "type": "int", "label": "$MSG_QUALITY", "default": $DEFAULT_QUALITY,
     "min": 0, "max": 100},
    {"id": "compression", "type": "int", "label": "$MSG_COMPRESSION", "default": $DEFAULT_COMPRESSION,
     "min": 0, "max": 6, "hint": "$MSG_COMPRESSION_HINT"}
  ]
}
EOF
}

# ---------------------------------------------------------------- phase 1 ---
if [ "${MMT_PHASE:-}" != "run" ]; then
    [ $# -gt 0 ] || exit 0

    # Exit status 1 = cancelled, 2 = error (mmt-dialog has already shown it)
    ANSWERS=$(build_form | "$SCRIPT_DIR/mmt-dialog") || exit 0
    SIZE=$(sed -n 's/^size=//p' <<< "$ANSWERS")
    FPS=$(sed -n 's/^fps=//p' <<< "$ANSWERS")
    QUALITY=$(sed -n 's/^quality=//p' <<< "$ANSWERS")
    COMPRESSION=$(sed -n 's/^compression=//p' <<< "$ANSWERS")

    exec "$SCRIPT_DIR/mmt-terminal" env MMT_PHASE=run \
        MMT_SIZE="$SIZE" MMT_FPS="$FPS" MMT_QUALITY="$QUALITY" MMT_COMPRESSION="$COMPRESSION" \
        "$SELF" "$@"
fi

# ---------------------------------------------------------------- phase 2 ---
SIZE="${MMT_SIZE:-$DEFAULT_SIZE}"
FPS="${MMT_FPS:-$DEFAULT_FPS}"
QUALITY="${MMT_QUALITY:-$DEFAULT_QUALITY}"
COMPRESSION="${MMT_COMPRESSION:-$DEFAULT_COMPRESSION}"

# Limit the shorter side to $SIZE, keep the aspect ratio, never upscale.
# Landscape: height = min(ih, SIZE), width follows (-2 keeps it even).
# Portrait / square: width = min(iw, SIZE), height follows.
SCALE="scale=w='if(gt(iw,ih),-2,min(iw,$SIZE))':h='if(gt(iw,ih),min(ih,$SIZE),-2)':flags=lanczos"

echo "=== $MSG_HEADER ${SIZE}px | ${FPS} fps | quality ${QUALITY} | compression ${COMPRESSION} ==="
echo

FAILED=0
for file in "$@"; do
    [ -f "$file" ] || continue
    # -an: drop audio, -loop 0: loop forever
    if ! ffmpeg -i "$file" -an \
            -vf "fps=$FPS,$SCALE" \
            -c:v libwebp -quality "$QUALITY" -compression_level "$COMPRESSION" \
            -loop 0 \
            "${file%.*}.webp"; then
        echo
        echo "[!] $MSG_FAILED $file"
        FAILED=1
    fi
done

# Keep the window open on errors, otherwise the message would vanish instantly
if [ "$FAILED" -ne 0 ]; then
    echo
    read -r -p "$MSG_PRESS_ENTER"
    exit 1
fi
