#!/bin/bash
# compress_mp4.sh - Compress video(s) to MP4.
#
# Asks for a compression level (low / medium / high) and a video codec in a
# single dialog, then runs ffmpeg in a terminal window.
#
# Usage: compress_mp4.sh file1 [file2 ...]
#
# The script runs in two phases:
#   1. (no terminal) show the dialog (mmt-dialog)
#   2. (MMT_PHASE=run) re-launch itself via mmt-terminal and encode

SELF="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(dirname "$SELF")"

# shellcheck source=mmt-gpu.sh
source "$SCRIPT_DIR/mmt-gpu.sh"

# Detect user language, same logic as the installer (ru/uk/en)
_lang="${LANG:-${LC_MESSAGES:-en}}"
case "${_lang%%_*}" in
    ru|be|kk)
        MSG_TITLE="Сжать в MP4"
        MSG_TAB_TITLE="Пресеты"
        MSG_TAB_TEXT="Фиксированное качество: выберите уровень сжатия, размер файла зависит от видео."
        MSG_RES_PROMPT="Разрешение:"
        MSG_RES_ORIGINAL="Оригинал (без изменений)"
        MSG_RES_GROUP_SIZE="По меньшей стороне"
        MSG_RES_GROUP_SCALE="Масштаб"
        MSG_RES_HINT="Можно ввести своё значение: 50%, x0.5 или 540p. Увеличения нет."
        MSG_RES_INVALID="Не распознано. Примеры: 50%, x0.5, 540p (без увеличения)."
        MSG_RES_UNCHANGED="без изменений"
        MSG_AUDIO_WORD="звук"
        MSG_MORE="ещё"
        MSG_LEVEL_PROMPT="Уровень сжатия:"
        MSG_LEVEL_LOW="Низкий — лучшее качество, файл больше"
        MSG_LEVEL_MEDIUM="Средний — баланс качества и размера"
        MSG_LEVEL_HIGH="Высокий — минимальный размер, качество ниже"
        MSG_CODEC_PROMPT="Видеокодек:"
        MSG_AV1_FAST="Быстрый"
        MSG_AV1_SLOW="Медленный"
        MSG_HEADER="Сжатие в MP4:"
        MSG_LEVEL_WORD="уровень"
        MSG_FAILED="Не удалось обработать:"
        MSG_PRESS_ENTER="Нажмите Enter, чтобы закрыть окно..."
        MSG_NO_GPU="Не найдена видеокарта:"
        MSG_GPU_ACCESS="Нет доступа к устройству видеокарты (добавьте пользователя в группу render):"
        ;;
    uk)
        MSG_TITLE="Стиснути в MP4"
        MSG_TAB_TITLE="Пресети"
        MSG_TAB_TEXT="Фіксована якість: оберіть рівень стиснення, розмір файлу залежить від відео."
        MSG_RES_PROMPT="Роздільна здатність:"
        MSG_RES_ORIGINAL="Оригінал (без змін)"
        MSG_RES_GROUP_SIZE="За меншою стороною"
        MSG_RES_GROUP_SCALE="Масштаб"
        MSG_RES_HINT="Можна ввести своє значення: 50%, x0.5 або 540p. Збільшення немає."
        MSG_RES_INVALID="Не розпізнано. Приклади: 50%, x0.5, 540p (без збільшення)."
        MSG_RES_UNCHANGED="без змін"
        MSG_AUDIO_WORD="звук"
        MSG_MORE="ще"
        MSG_LEVEL_PROMPT="Рівень стиснення:"
        MSG_LEVEL_LOW="Низький — найкраща якість, файл більший"
        MSG_LEVEL_MEDIUM="Середній — баланс якості та розміру"
        MSG_LEVEL_HIGH="Високий — мінімальний розмір, якість нижча"
        MSG_CODEC_PROMPT="Відеокодек:"
        MSG_AV1_FAST="Швидкий"
        MSG_AV1_SLOW="Повільний"
        MSG_HEADER="Стиснення в MP4:"
        MSG_LEVEL_WORD="рівень"
        MSG_FAILED="Не вдалося обробити:"
        MSG_PRESS_ENTER="Натисніть Enter, щоб закрити вікно..."
        MSG_NO_GPU="Не знайдено відеокарту:"
        MSG_GPU_ACCESS="Немає доступу до пристрою відеокарти (додайте користувача до групи render):"
        ;;
    *)
        MSG_TITLE="Compress to MP4"
        MSG_TAB_TITLE="Presets"
        MSG_TAB_TEXT="Fixed quality: pick a compression level, the file size depends on the video."
        MSG_RES_PROMPT="Resolution:"
        MSG_RES_ORIGINAL="Original (no scaling)"
        MSG_RES_GROUP_SIZE="Shorter side"
        MSG_RES_GROUP_SCALE="Scale"
        MSG_RES_HINT="You can type your own value: 50%, x0.5 or 540p. Never scales up."
        MSG_RES_INVALID="Not recognised. Examples: 50%, x0.5, 540p (no upscaling)."
        MSG_RES_UNCHANGED="unchanged"
        MSG_AUDIO_WORD="audio"
        MSG_MORE="more"
        MSG_LEVEL_PROMPT="Compression level:"
        MSG_LEVEL_LOW="Low — best quality, larger file"
        MSG_LEVEL_MEDIUM="Medium — balanced quality and size"
        MSG_LEVEL_HIGH="High — smallest file, lower quality"
        MSG_CODEC_PROMPT="Video codec:"
        MSG_AV1_FAST="Fast"
        MSG_AV1_SLOW="Slow"
        MSG_HEADER="Compress to MP4:"
        MSG_LEVEL_WORD="level"
        MSG_FAILED="Failed to process:"
        MSG_PRESS_ENTER="Press Enter to close this window..."
        MSG_NO_GPU="No GPU found:"
        MSG_GPU_ACCESS="No access to the GPU device (add your user to the render group):"
        ;;
esac

# The form is described as JSON for mmt-dialog.
# Keep the MSG_* texts free of double quotes and backslashes (they go into JSON).
# The fields alone: they are the same for the stand-alone dialog and for the tab
# of the combined "Compress Video" dialog (compress_video.sh).
build_fields() {
    cat << EOF
    {"id": "level", "type": "combo", "label": "$MSG_LEVEL_PROMPT", "default": "medium",
     "items": [
       {"value": "low",    "text": "$MSG_LEVEL_LOW"},
       {"value": "medium", "text": "$MSG_LEVEL_MEDIUM"},
       {"value": "high",   "text": "$MSG_LEVEL_HIGH"}
     ]},
    {"id": "codec", "type": "combo", "label": "$MSG_CODEC_PROMPT", "default": "h264_cpu", "show_group": true,
     "items": [
       {"header": "CPU"},
       {"value": "h264_cpu",     "text": "H.264"},
       {"value": "h265_cpu",     "text": "HEVC"},
       {"value": "av1_cpu_fast", "text": "AV1 $MSG_AV1_FAST"},
       {"value": "av1_cpu_slow", "text": "AV1 $MSG_AV1_SLOW"},
       {"header": "GPU · Nvidia"},
       {"value": "h264_nv",      "text": "H.264"},
       {"value": "h265_nv",      "text": "HEVC"},
       {"value": "av1_nv",       "text": "AV1"},
       {"header": "GPU · AMD"},
       {"value": "h264_amd",     "text": "H.264"},
       {"value": "h265_amd",     "text": "HEVC"},
       {"value": "av1_amd",      "text": "AV1"},
       {"header": "GPU · Intel"},
       {"value": "h264_intel",   "text": "H.264"},
       {"value": "h265_intel",   "text": "HEVC"},
       {"value": "av1_intel",    "text": "AV1"}
     ]},
    {"id": "scale", "type": "scale", "label": "$MSG_RES_PROMPT", "default": "orig", "hint": "$MSG_RES_HINT",
     "texts": {"invalid": "$MSG_RES_INVALID", "unchanged": "$MSG_RES_UNCHANGED"},
     "items": [
       {"value": "orig", "text": "$MSG_RES_ORIGINAL"},
       {"header": "$MSG_RES_GROUP_SIZE"},
       {"value": "h1440", "text": "1440p"},
       {"value": "h1080", "text": "1080p"},
       {"value": "h720",  "text": "720p"},
       {"value": "h540",  "text": "540p"},
       {"value": "h480",  "text": "480p"},
       {"value": "h360",  "text": "360p"},
       {"header": "$MSG_RES_GROUP_SCALE"},
       {"value": "s0.75", "text": "75%"},
       {"value": "s0.5",  "text": "50%"},
       {"value": "s0.33", "text": "33%"},
       {"value": "s0.25", "text": "25%"}
     ]}
EOF
}

# Stand-alone dialog (this script started directly)
build_form() {
    cat << EOF
{
  "title": "$MSG_TITLE",
  "probe_texts": {"audio": "$MSG_AUDIO_WORD", "more": "$MSG_MORE"},
  "fields": [
$(build_fields)
  ]
}
EOF
}

# One tab of the combined dialog: compress_video.sh asks for it with --print-tab
build_tab() {
    cat << EOF
{
  "id": "preset", "title": "$MSG_TAB_TITLE", "text": "$MSG_TAB_TEXT",
  "probe_texts": {"audio": "$MSG_AUDIO_WORD", "more": "$MSG_MORE"},
  "fields": [
$(build_fields)
  ]
}
EOF
}

# ---------------------------------------------------------------- phase 1 ---
if [ "${MMT_PHASE:-}" != "run" ]; then
    # Used by compress_video.sh: print this script's tab of the combined dialog
    if [ "${1:-}" = "--print-tab" ]; then
        build_tab
        exit 0
    fi

    [ $# -gt 0 ] || exit 0

    # Exit status 1 = cancelled, 2 = error (mmt-dialog has already shown it)
    ANSWERS=$(build_form | "$SCRIPT_DIR/mmt-dialog" --probe "$@") || exit 0
    LEVEL=$(sed -n 's/^level=//p' <<< "$ANSWERS")
    CODEC=$(sed -n 's/^codec=//p' <<< "$ANSWERS")
    SCALE=$(sed -n 's/^scale=//p' <<< "$ANSWERS")

    exec "$SCRIPT_DIR/mmt-terminal" env MMT_PHASE=run MMT_LEVEL="$LEVEL" MMT_CODEC="$CODEC" MMT_SCALE="$SCALE" "$SELF" "$@"
fi

# ---------------------------------------------------------------- phase 2 ---
LEVEL="${MMT_LEVEL:-medium}"
CODEC="${MMT_CODEC:-h264_cpu}"
# Resolution: orig | h<px of the shorter side> | s<factor>
SCALE_MODE="${MMT_SCALE:-orig}"
[[ "$SCALE_MODE" =~ ^(orig|h[0-9]+|s[0-9]*[.]?[0-9]+)$ ]] || SCALE_MODE=orig

case "$LEVEL" in
    low)    IDX=0 ;;
    medium) IDX=1 ;;
    high)   IDX=2 ;;
    *)      echo "Unknown compression level: $LEVEL" >&2; exit 1 ;;
esac

# pick "<low> <medium> <high>" -> the value for the selected level
pick() {
    local -a values=($1)
    echo "${values[$IDX]}"
}

# Values per level (low / medium / high) are the original
# "Compress to MP4" / "MP4+" / "MP4++" settings.
# Print an error and keep the window open so the message does not vanish
die() {
    echo "[!] $*" >&2
    echo
    read -r -p "$MSG_PRESS_ENTER"
    exit 1
}

# AMD and Intel GPUs encode through VAAPI. The video is decoded on the CPU and
# uploaded to the GPU (works with any source codec); the render device of the
# requested vendor is looked up, so computers with several GPUs pick the right one.
vaapi_setup() {   # amd | intel
    local node
    GPU_NAME="Intel"
    [ "$1" = "amd" ] && GPU_NAME="AMD"
    node=$(mmt_render_node "$1") || die "$MSG_NO_GPU $GPU_NAME"
    { [ -r "$node" ] && [ -w "$node" ]; } || die "$MSG_GPU_ACCESS $node"
    PRE=(-vaapi_device "$node")
    HWFILTER="format=nv12,hwupload"   # added after the (software) scaling filter
}

PRE=()
HWFILTER=""
case "$CODEC" in
    h264_cpu)
        LABEL="H.264 [CPU]"
        ENC=(-pix_fmt yuv420p -c:v libx264 -profile:v main -crf "$(pick '22 30 35')")
        ;;
    h264_nv)
        LABEL="H.264 [GPU] Nvidia"
        PRE=(-hwaccel cuda -hwaccel_output_format cuda)
        ENC=(-c:v h264_nvenc -profile:v main -cq "$(pick '29 37 41')" -preset p7)
        ;;
    h265_cpu)
        LABEL="HEVC [CPU]"
        ENC=(-pix_fmt yuv420p -c:v libx265 -profile:v main -crf "$(pick '22 30 35')" -tag:v hvc1)
        ;;
    h265_nv)
        LABEL="HEVC [GPU] Nvidia"
        PRE=(-hwaccel cuda -hwaccel_output_format cuda)
        ENC=(-c:v hevc_nvenc -profile:v main -cq "$(pick '28 37 43')" -preset p7 -tag:v hvc1)
        ;;
    av1_cpu_fast)
        LABEL="AV1 [CPU] $MSG_AV1_FAST"
        ENC=(-c:v libsvtav1 -preset "$(pick '7 7 5')" -rc vbr -crf "$(pick '32 44 55')")
        ;;
    av1_cpu_slow)
        LABEL="AV1 [CPU] $MSG_AV1_SLOW"
        ENC=(-c:v libsvtav1 -preset "$(pick '5 5 4')" -rc vbr -crf "$(pick '32 44 55')")
        ;;
    av1_nv)
        LABEL="AV1 [GPU] Nvidia"
        PRE=(-hwaccel cuda -hwaccel_output_format cuda)
        ENC=(-c:v av1_nvenc -cq "$(pick '37 47 56')" -preset p7)
        ;;
    h264_amd|h264_intel)
        vaapi_setup "${CODEC##*_}"
        LABEL="H.264 [GPU] $GPU_NAME"
        ENC=(-c:v h264_vaapi -profile:v main -global_quality "$(pick '29 37 41')")
        ;;
    h265_amd|h265_intel)
        vaapi_setup "${CODEC##*_}"
        LABEL="HEVC [GPU] $GPU_NAME"
        ENC=(-c:v hevc_vaapi -profile:v main -global_quality "$(pick '28 37 43')" -tag:v hvc1)
        ;;
    av1_amd|av1_intel)
        vaapi_setup "${CODEC##*_}"
        LABEL="AV1 [GPU] $GPU_NAME"
        ENC=(-c:v av1_vaapi -global_quality "$(pick '37 47 56')")
        ;;
    *)
        echo "Unknown codec: $CODEC" >&2
        exit 1
        ;;
esac

echo "=== $MSG_HEADER $LABEL | $MSG_LEVEL_WORD: $LEVEL ==="
echo

FAILED=0
for file in "$@"; do
    [ -f "$file" ] || continue

    # --- Resolution. The scale factor is relative to the SHORTER side, so portrait
    # videos behave like landscape ones; nothing is ever scaled up.
    PRE_EFF=("${PRE[@]}")
    SCALE_FILTER=""
    SCALE_INFO=""
    if [ "$SCALE_MODE" != "orig" ]; then
        # (width and height are asked separately: for rotated videos ffprobe adds a stray
        # separator to a combined CSV line)
        SRC_W=$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null | head -n1)
        SRC_H=$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null | head -n1)
        if [[ "$SRC_W" =~ ^[0-9]+$ ]] && [[ "$SRC_H" =~ ^[0-9]+$ ]]; then
            read -r SCALE_F SCALE_SHORT < <(LC_ALL=C awk -v mode="$SCALE_MODE" -v w="$SRC_W" -v h="$SRC_H" 'BEGIN {
                short = (w < h) ? w : h
                f = 1
                if (mode ~ /^s/)      f = substr(mode, 2) + 0
                else if (mode ~ /^h/) f = (substr(mode, 2) + 0) / short
                if (f > 1 || f >= 0.999) f = 1
                printf "%.6f %d", f, 2 * int(short * f / 2 + 0.5)
            }')
            if LC_ALL=C awk -v f="$SCALE_F" 'BEGIN { exit !(f < 1) }'; then
                SCALE_FILTER="scale=2*round(iw*${SCALE_F}/2):2*round(ih*${SCALE_F}/2):flags=lanczos"
                SCALE_INFO=" | scale x${SCALE_F} (shorter side ${SCALE_SHORT}px)"
                # GPU frames cannot go through the software scaler: decode on the GPU, scale in memory
                case "$CODEC" in
                    *_nv) PRE_EFF=(-hwaccel cuda) ;;
                esac
            fi
        fi
    fi

    # Filter chain: software scaling first, then (VAAPI only) upload the frames to the GPU
    VIDEO_FILTERS="$SCALE_FILTER"
    if [ -n "$HWFILTER" ]; then
        VIDEO_FILTERS="${VIDEO_FILTERS:+$VIDEO_FILTERS,}$HWFILTER"
    fi
    VF_ARGS=()
    if [ -n "$VIDEO_FILTERS" ]; then
        VF_ARGS=(-vf "$VIDEO_FILTERS")
    fi

    [ -z "$SCALE_INFO" ] || echo "-> $(basename "$file")${SCALE_INFO}"
    if ! ffmpeg "${PRE_EFF[@]}" -i "$file" "${VF_ARGS[@]}" "${ENC[@]}" "${file%.*}_compressed.mp4"; then
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
