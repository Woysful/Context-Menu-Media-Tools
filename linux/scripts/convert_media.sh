#!/bin/bash
# convert_media.sh - Convert / extract / split video(s); the output format is
# chosen from a drop-down list in a dialog.
#
# Usage: convert_media.sh <mode> file1 [file2 ...]
#
# Modes:
#   video    Convert Video         -> MP4, AVI, MOV, MKV, M4V, WebM
#   audio    Convert Audio         -> re-encode the audio tracks, video is copied
#   extract  Extract Audio         -> MP3, AAC, Opus, WAV, M4A, MKA
#   frames   Make frame sequence   -> PNG, JPG, WebP images in a new folder
#
# The script runs in two phases:
#   1. (no terminal) show the dialog (mmt-dialog)
#   2. (MMT_PHASE=run) re-launch itself via mmt-terminal and run ffmpeg

SELF="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(dirname "$SELF")"

MODE="$1"
shift

case "$MODE" in
    video|audio|extract|frames) ;;
    *)
        echo "Usage: $(basename "$0") video|audio|extract|frames FILE..." >&2
        exit 2
        ;;
esac

# Detect user language, same logic as the installer (ru/uk/en)
_lang="${LANG:-${LC_MESSAGES:-en}}"
case "${_lang%%_*}" in
    ru|be|kk)
        MSG_FORMAT_PROMPT="Выходной формат:"
        MSG_FAILED="Не удалось обработать:"
        MSG_PRESS_ENTER="Нажмите Enter, чтобы закрыть окно..."
        case "$MODE" in
            video)   MSG_TITLE="Конвертировать видео" ;;
            audio)   MSG_TITLE="Конвертировать аудио" ;;
            extract) MSG_TITLE="Извлечь аудио" ;;
            frames)  MSG_TITLE="Разбить на кадры" ;;
        esac
        ;;
    uk)
        MSG_FORMAT_PROMPT="Вихідний формат:"
        MSG_FAILED="Не вдалося обробити:"
        MSG_PRESS_ENTER="Натисніть Enter, щоб закрити вікно..."
        case "$MODE" in
            video)   MSG_TITLE="Конвертувати відео" ;;
            audio)   MSG_TITLE="Конвертувати аудіо" ;;
            extract) MSG_TITLE="Витягнути аудіо" ;;
            frames)  MSG_TITLE="Розбити на кадри" ;;
        esac
        ;;
    *)
        MSG_FORMAT_PROMPT="Output format:"
        MSG_FAILED="Failed to process:"
        MSG_PRESS_ENTER="Press Enter to close this window..."
        case "$MODE" in
            video)   MSG_TITLE="Convert Video" ;;
            audio)   MSG_TITLE="Convert Audio" ;;
            extract) MSG_TITLE="Extract Audio" ;;
            frames)  MSG_TITLE="Make frame sequence" ;;
        esac
        ;;
esac

# Drop-down entries and the preselected one for each mode
case "$MODE" in
    video)
        DEFAULT_FORMAT="mp4"
        ITEMS='{"value": "mp4",  "text": "MP4"},
       {"value": "avi",  "text": "AVI"},
       {"value": "mov",  "text": "MOV"},
       {"value": "mkv",  "text": "MKV"},
       {"value": "m4v",  "text": "M4V"},
       {"value": "webm", "text": "WebM"}'
        ;;
    audio)
        DEFAULT_FORMAT="mp3"
        ITEMS='{"value": "wav",  "text": "WAV"},
       {"value": "mp3",  "text": "MP3"},
       {"value": "flac", "text": "FLAC"},
       {"value": "aac",  "text": "AAC"},
       {"value": "ogg",  "text": "OGG"},
       {"value": "wma",  "text": "WMA"},
       {"value": "m4a",  "text": "M4A"},
       {"value": "opus", "text": "Opus"}'
        ;;
    extract)
        DEFAULT_FORMAT="mp3"
        ITEMS='{"value": "mp3",  "text": "MP3"},
       {"value": "aac",  "text": "AAC"},
       {"value": "opus", "text": "Opus"},
       {"value": "wav",  "text": "WAV"},
       {"value": "m4a",  "text": "M4A"},
       {"value": "mka",  "text": "MKA"}'
        ;;
    frames)
        DEFAULT_FORMAT="png"
        ITEMS='{"value": "png",  "text": "PNG"},
       {"value": "jpg",  "text": "JPG"},
       {"value": "webp", "text": "WebP"}'
        ;;
esac

# The form is described as JSON for mmt-dialog.
# Keep the MSG_* texts free of double quotes and backslashes (they go into JSON).
build_form() {
    cat << EOF
{
  "title": "$MSG_TITLE",
  "min_width": 360,
  "fields": [
    {"id": "format", "type": "combo", "label": "$MSG_FORMAT_PROMPT", "default": "$DEFAULT_FORMAT",
     "items": [
       $ITEMS
     ]}
  ]
}
EOF
}

# ---------------------------------------------------------------- phase 1 ---
if [ "${MMT_PHASE:-}" != "run" ]; then
    [ $# -gt 0 ] || exit 0

    # Exit status 1 = cancelled, 2 = error (mmt-dialog has already shown it)
    ANSWERS=$(build_form | "$SCRIPT_DIR/mmt-dialog") || exit 0
    FORMAT=$(sed -n 's/^format=//p' <<< "$ANSWERS")

    exec "$SCRIPT_DIR/mmt-terminal" env MMT_PHASE=run MMT_FORMAT="$FORMAT" "$SELF" "$MODE" "$@"
fi

# ---------------------------------------------------------------- phase 2 ---
FORMAT="${MMT_FORMAT:-$DEFAULT_FORMAT}"

# ffmpeg arguments before the per-format ones (PRE) and per format (ARGS);
# EXT is the output extension. These are the settings the old submenus used.
PRE=()
ARGS=()
EXT="$FORMAT"

case "$MODE:$FORMAT" in
    video:mp4)   ARGS=(-c:v copy -c:a aac) ;;
    video:avi)   ARGS=(-c:v mpeg4 -q:v 2) ;;
    video:mov)   ARGS=(-c:v copy) ;;
    video:mkv)   ARGS=(-c:v copy -c:a copy) ;;
    video:m4v)   ARGS=(-c:v copy -c:a aac) ;;
    video:webm)  ARGS=(-c:v libvpx-vp9 -c:a libopus) ;;

    audio:*)     PRE=(-map 0:v -c:v copy -map 0:a)
                 case "$FORMAT" in
                     wav)  ARGS=(-c:a pcm_s16le) ;;
                     mp3)  ARGS=(-c:a mp3 -b:a 192k) ;;
                     flac) ARGS=(-c:a flac) ;;
                     aac)  ARGS=(-c:a aac -b:a 192k) ;;
                     ogg)  ARGS=(-c:a libvorbis -q:a 0) ;;
                     wma)  ARGS=(-c:a wmav2 -b:a 192k) ;;
                     m4a)  ARGS=(-c:a aac -b:a 192k) ;;
                     opus) ARGS=(-c:a libopus -q:a 0) ;;
                     *)    echo "Unknown format: $FORMAT" >&2; exit 1 ;;
                 esac ;;

    extract:mp3)  ARGS=(-vn -q:a 0) ;;
    extract:aac)  ARGS=(-vn -c copy) ;;
    extract:opus) ARGS=(-vn -c:a libopus -q:a 0 -vbr on) ;;
    extract:wav)  ARGS=(-vn -q:a 0) ;;
    extract:m4a)  ARGS=(-vn -q:a 0 -map a -map_metadata 0) ;;
    extract:mka)  ARGS=(-vn -c:a copy -map a -map_metadata 0) ;;

    frames:png)   ARGS=() ;;
    frames:jpg)   ARGS=(-q:v 1) ;;
    frames:webp)  ARGS=(-c:v libwebp -q:v 95 -pix_fmt yuva420p) ;;

    *)  echo "Unknown format: $MODE:$FORMAT" >&2; exit 1 ;;
esac

echo "=== $MSG_TITLE: ${FORMAT^^} ==="
echo

FAILED=0
for file in "$@"; do
    [ -f "$file" ] || continue
    base="${file%.*}"

    case "$MODE" in
        audio)
            # Keeps the container of the source file, e.g. "clip_audio_mp3.mkv"
            output="${base}_audio_${FORMAT}.${file##*.}"
            ;;
        frames)
            mkdir -p "$base"
            output="${base}/frame_%06d.${EXT}"
            ;;
        *)
            output="${base}.${EXT}"
            ;;
    esac

    if ! ffmpeg -i "$file" "${PRE[@]}" "${ARGS[@]}" "$output"; then
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
