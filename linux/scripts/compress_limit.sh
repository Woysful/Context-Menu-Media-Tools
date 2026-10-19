#!/bin/bash
# compress_limit.sh - Compress video(s) to a target size budget in a single encoding pass.
#
# Asks for the size limit and the video codec in a single dialog (the codec
# list is split into Preset / CPU / GPU), then runs ffmpeg in a terminal window.
#
# Usage: compress_limit.sh file1 [file2 ...]
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
        MSG_TITLE="Сжать до лимита"
        MSG_TAB_TITLE="Лимит размера"
        MSG_TAB_TEXT="Уложить результат в заданный размер файла (мессенджеры, Discord, почта)."
        MSG_LIMIT_PROMPT="Лимит размера итогового файла:"
        MSG_CODEC_PROMPT="Видеокодек:"
        MSG_GROUP_PRESET="Пресет"
        MSG_TWOPASS="Два прохода (точнее размер)"
        MSG_TWOPASS_HINT="Дольше по времени. Только для H.264 / HEVC [CPU]. Первый проход лишь анализирует видео, перекодирования дважды нет."
        MSG_RES_PROMPT="Разрешение:"
        MSG_RES_ORIGINAL="Оригинал (без изменений)"
        MSG_RES_AUTO="Авто (подобрать под лимит)"
        MSG_RES_GROUP_SIZE="По меньшей стороне"
        MSG_RES_GROUP_SCALE="Масштаб"
        MSG_RES_HINT="Можно ввести своё значение: 50%, x0.5 или 540p. Увеличения нет."
        MSG_RES_INVALID="Не распознано. Примеры: 50%, x0.5, 540p (без увеличения)."
        MSG_RES_AUTO_HINT="Для каждого файла выбирается самый большой размер, при котором при этом лимите на пиксель остаётся достаточно бит."
        MSG_RES_UNCHANGED="без изменений"
        MSG_AUDIO_WORD="звук"
        MSG_MORE="ещё"
        MSG_INVALID_LIMIT="Некорректный лимит размера:"
        MSG_UNKNOWN_CODEC="Неизвестный кодек:"
        MSG_HEADER="Сжатие до лимита:"
        MSG_BUDGET="лимит на файл:"
        MSG_TIGHT="Лимит слишком мал для такой длительности, качество будет очень низким."
        MSG_DONE="Готово:"
        MSG_TARGET="цель"
        MSG_OVER="Файл превышает лимит на"
        MSG_FAILED="Не удалось обработать:"
        MSG_PRESS_ENTER="Нажмите Enter, чтобы закрыть окно..."
        MSG_NO_GPU="Не найдена видеокарта:"
        MSG_GPU_ACCESS="Нет доступа к устройству видеокарты (добавьте пользователя в группу render):"
        ;;
    uk)
        MSG_TITLE="Стиснути до ліміту"
        MSG_TAB_TITLE="Ліміт розміру"
        MSG_TAB_TEXT="Вкластися у заданий розмір файлу (месенджери, Discord, пошта)."
        MSG_LIMIT_PROMPT="Ліміт розміру підсумкового файлу:"
        MSG_CODEC_PROMPT="Відеокодек:"
        MSG_GROUP_PRESET="Пресет"
        MSG_TWOPASS="Два проходи (точніший розмір)"
        MSG_TWOPASS_HINT="Довше за часом. Лише для H.264 / HEVC [CPU]. Перший прохід лише аналізує відео, перекодування двічі немає."
        MSG_RES_PROMPT="Роздільна здатність:"
        MSG_RES_ORIGINAL="Оригінал (без змін)"
        MSG_RES_AUTO="Авто (підібрати під ліміт)"
        MSG_RES_GROUP_SIZE="За меншою стороною"
        MSG_RES_GROUP_SCALE="Масштаб"
        MSG_RES_HINT="Можна ввести своє значення: 50%, x0.5 або 540p. Збільшення немає."
        MSG_RES_INVALID="Не розпізнано. Приклади: 50%, x0.5, 540p (без збільшення)."
        MSG_RES_AUTO_HINT="Для кожного файлу обирається найбільший розмір, за якого при цьому ліміті на піксель лишається достатньо бітів."
        MSG_RES_UNCHANGED="без змін"
        MSG_AUDIO_WORD="звук"
        MSG_MORE="ще"
        MSG_INVALID_LIMIT="Некоректний ліміт розміру:"
        MSG_UNKNOWN_CODEC="Невідомий кодек:"
        MSG_HEADER="Стиснення до ліміту:"
        MSG_BUDGET="ліміт на файл:"
        MSG_TIGHT="Ліміт занадто малий для такої тривалості, якість буде дуже низькою."
        MSG_DONE="Готово:"
        MSG_TARGET="ціль"
        MSG_OVER="Файл перевищує ліміт на"
        MSG_FAILED="Не вдалося обробити:"
        MSG_PRESS_ENTER="Натисніть Enter, щоб закрити вікно..."
        MSG_NO_GPU="Не знайдено відеокарту:"
        MSG_GPU_ACCESS="Немає доступу до пристрою відеокарти (додайте користувача до групи render):"
        ;;
    *)
        MSG_TITLE="Compress to Limit"
        MSG_TAB_TITLE="Size limit"
        MSG_TAB_TEXT="Fit the result into a file size limit (messengers, Discord, e-mail)."
        MSG_LIMIT_PROMPT="Size limit of the final file:"
        MSG_CODEC_PROMPT="Video codec:"
        MSG_GROUP_PRESET="Preset"
        MSG_TWOPASS="Two-pass encoding (more accurate size)"
        MSG_TWOPASS_HINT="Takes longer. Only for H.264 / HEVC [CPU]. The first pass only analyses the video, nothing is encoded twice."
        MSG_RES_PROMPT="Resolution:"
        MSG_RES_ORIGINAL="Original (no scaling)"
        MSG_RES_AUTO="Auto (fit to the limit)"
        MSG_RES_GROUP_SIZE="Shorter side"
        MSG_RES_GROUP_SCALE="Scale"
        MSG_RES_HINT="You can type your own value: 50%, x0.5 or 540p. Never scales up."
        MSG_RES_INVALID="Not recognised. Examples: 50%, x0.5, 540p (no upscaling)."
        MSG_RES_AUTO_HINT="For each file the largest size that still gets enough bits per pixel at this limit is chosen."
        MSG_RES_UNCHANGED="unchanged"
        MSG_AUDIO_WORD="audio"
        MSG_MORE="more"
        MSG_INVALID_LIMIT="Invalid size limit:"
        MSG_UNKNOWN_CODEC="Unknown codec:"
        MSG_HEADER="Compress to Limit:"
        MSG_BUDGET="budget per file:"
        MSG_TIGHT="Budget is very tight for this duration, quality will be poor."
        MSG_DONE="Done:"
        MSG_TARGET="target"
        MSG_OVER="File is over the limit by"
        MSG_FAILED="Failed to process:"
        MSG_PRESS_ENTER="Press Enter to close this window..."
        MSG_NO_GPU="No GPU found:"
        MSG_GPU_ACCESS="No access to the GPU device (add your user to the render group):"
        ;;
esac

# The form is described as JSON for mmt-dialog.
# Keep the MSG_* texts free of double quotes and backslashes (they go into JSON).
# Choosing the Discord preset fixes the size limit field to 20 MB.
# The fields alone: they are the same for the stand-alone dialog and for the tab
# of the combined "Compress Video" dialog (compress_video.sh).
build_fields() {
    cat << EOF
    {"id": "limit", "type": "float", "label": "$MSG_LIMIT_PROMPT", "default": 20,
     "min": 0.1, "max": 100000, "decimals": 2, "suffix": " MB"},
    {"id": "codec", "type": "combo", "label": "$MSG_CODEC_PROMPT", "default": "h264_cpu", "show_group": true,
     "items": [
       {"header": "$MSG_GROUP_PRESET", "no_suffix": true},
       {"value": "discord",    "text": "Discord — 20 MB, HEVC (CPU)", "lock": {"limit": 20}},
       {"header": "CPU"},
       {"value": "h264_cpu",   "text": "H.264"},
       {"value": "h265_cpu",   "text": "HEVC"},
       {"value": "av1_cpu",    "text": "AV1"},
       {"header": "GPU · Nvidia"},
       {"value": "h264_nv",    "text": "H.264"},
       {"value": "h265_nv",    "text": "HEVC"},
       {"value": "av1_nv",     "text": "AV1"},
       {"header": "GPU · AMD"},
       {"value": "h264_amd",   "text": "H.264"},
       {"value": "h265_amd",   "text": "HEVC"},
       {"value": "av1_amd",    "text": "AV1"},
       {"header": "GPU · Intel"},
       {"value": "h264_intel", "text": "H.264"},
       {"value": "h265_intel", "text": "HEVC"},
       {"value": "av1_intel",  "text": "AV1"}
     ]},
    {"id": "scale", "type": "scale", "label": "$MSG_RES_PROMPT", "default": "orig", "hint": "$MSG_RES_HINT",
     "texts": {"invalid": "$MSG_RES_INVALID", "auto": "$MSG_RES_AUTO_HINT", "unchanged": "$MSG_RES_UNCHANGED"},
     "items": [
       {"value": "orig", "text": "$MSG_RES_ORIGINAL"},
       {"value": "auto", "text": "$MSG_RES_AUTO"},
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
     ]},
    {"id": "twopass", "type": "check", "label": "$MSG_TWOPASS", "default": false, "hint": "$MSG_TWOPASS_HINT"}
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
  "id": "limit", "title": "$MSG_TAB_TITLE", "text": "$MSG_TAB_TEXT",
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
    # --probe makes the dialog show resolution / bitrate of the selected files
    ANSWERS=$(build_form | "$SCRIPT_DIR/mmt-dialog" --probe "$@") || exit 0
    BUDGET_MB=$(sed -n 's/^limit=//p' <<< "$ANSWERS")
    CODEC_KEY=$(sed -n 's/^codec=//p' <<< "$ANSWERS")
    TWOPASS=$(sed -n 's/^twopass=//p' <<< "$ANSWERS")
    SCALE=$(sed -n 's/^scale=//p' <<< "$ANSWERS")

    exec "$SCRIPT_DIR/mmt-terminal" env MMT_PHASE=run MMT_BUDGET="$BUDGET_MB" MMT_CODEC="$CODEC_KEY" MMT_TWOPASS="$TWOPASS" MMT_SCALE="$SCALE" "$SELF" "$@"
fi

# ---------------------------------------------------------------- phase 2 ---
CODEC_KEY="${MMT_CODEC:-h264_cpu}"
BUDGET_MB="${MMT_BUDGET:-20}"
# Resolution: orig | auto | h<px of the shorter side> | s<factor>
SCALE_MODE="${MMT_SCALE:-orig}"
[[ "$SCALE_MODE" =~ ^(orig|auto|h[0-9]+|s[0-9]*[.]?[0-9]+)$ ]] || SCALE_MODE=orig
FILES=("$@")

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
USE_VAAPI=0
vaapi_setup() {   # amd | intel
    local node
    GPU_NAME="Intel"
    [ "$1" = "amd" ] && GPU_NAME="AMD"
    node=$(mmt_render_node "$1") || die "$MSG_NO_GPU $GPU_NAME"
    { [ -r "$node" ] && [ -w "$node" ]; } || die "$MSG_GPU_ACCESS $node"
    PRE=(-vaapi_device "$node")
    USE_VAAPI=1
}

# Closed-loop encode for hardware encoders (NVENC / VAAPI). They only approximate
# the requested bitrate, by an amount that depends on the GPU, the driver and the
# footage, so a bitrate computed in advance can miss the size limit by several percent.
# Here the video is cut into chunks and every chunk is encoded exactly once; after each
# one the real size is measured and the bitrate of the remaining chunks is corrected by
# what the encoder actually did. The chunks are then joined without re-encoding.
# The last chunk is short (its error is the one that stays in the result).
# Uses the per-file variables of the main loop; returns 1 if ffmpeg fails.
encode_chunked() {
    local work list n len i start dur_i remaining_bytes remaining_dur
    local target_bps req_bytes actual sum_req sum_actual audio_bytes ratio_note idx x skip
    local vf limit pre_chunk=() tag_args=() audio_in=() audio_map=()
    # Narrow rate window per chunk: a chunk may not run far over what was requested for it
    # (a smaller chunk is harmless, the unused bytes go to the next ones)
    local rc_max=110 rc_buf=100

    work=$(mktemp -d) || return 1
    list="$work/chunks.txt"

    # Frames stay in normal memory here (trim / setpts below are software filters),
    # so the "keep decoded frames on the GPU" option of the Nvidia path is dropped
    skip=0
    for x in "${PRE_EFF[@]}"; do
        if [ "$skip" -eq 1 ]; then
            skip=0
        elif [ "$x" = "-hwaccel_output_format" ]; then
            skip=1
        else
            pre_chunk+=("$x")
        fi
    done

    # Audio is encoded once; afterwards its size is known exactly
    audio_bytes=0
    if [ "$HAS_AUDIO" -eq 1 ]; then
        if ! ffmpeg -y -hide_banner -loglevel error -i "$file" -vn "${AUDIO_ARGS[@]}" "$work/audio.m4a"; then
            rm -rf "$work"
            return 1
        fi
        audio_bytes=$(stat -c %s "$work/audio.m4a")
        audio_in=(-i "$work/audio.m4a")
        audio_map=(-map 1:a)
    fi

    # What is left for the video: the limit (times the aim), minus the container
    # overhead (estimated from the frame counts) and the finished audio track
    remaining_bytes=$(LC_ALL=C awk -v mb="$BUDGET_MB" -v aim="$AIM_EFF" -v dur="$DURATION" -v fps="$FPS" -v aud="$HAS_AUDIO" -v ab="$audio_bytes" 'BEGIN {
        printf "%.0f", mb * 1048576 * aim / 100 - (4096 + dur * fps * 12 + (aud ? dur * 47 * 20 : 0)) - ab
    }')
    remaining_dur="$DURATION"

    # Chunk plan: equal chunks of about 6 s (3 to 16 of them) and a short last one
    read -r n len _ < <(LC_ALL=C awk -v d="$DURATION" 'BEGIN {
        last = d / 12; if (last < 1) last = 1; if (last > 3) last = 3
        m = int((d - last) / 6 + 0.5); if (m < 3) m = 3; if (m > 16) m = 16
        printf "%d %.6f %.6f", m + 1, (d - last) / m, last
    }')
    sum_req=0
    sum_actual=0

    for ((i = 0; i < n; i++)); do
        start=$(LC_ALL=C awk -v l="$len" -v i="$i" 'BEGIN { printf "%.6f", l * i }')
        # Cut by absolute time with the trim filter (every frame lands in exactly one
        # chunk) and restart the time stamps at 0 so the chunks join seamlessly.
        # -t only stops reading soon after the cut, it is not the cut itself.
        if [ "$i" -lt $((n - 1)) ]; then
            dur_i="$len"
            vf="trim=end=${len},setpts=PTS-STARTPTS"
            limit=(-t "$(LC_ALL=C awk -v l="$len" 'BEGIN { printf "%.6f", l + 1 }')")
        else
            dur_i="$remaining_dur"          # the last chunk runs to the end of the video
            vf="setpts=PTS-STARTPTS"
            limit=()
        fi
        if [ -n "$VIDEO_FILTERS" ]; then
            vf="$vf,$VIDEO_FILTERS"
        fi

        # Fair share of what is left, divided by how much more than requested the
        # encoder has produced so far (1 for the first chunk). Running over the limit is
        # worse than falling short, so an encoder that tends to produce less than
        # requested is only compensated a little (not at all in the last chunk, which
        # can no longer be corrected), while one that produces more is compensated fully.
        read -r target_bps req_bytes < <(LC_ALL=C awk -v rb="$remaining_bytes" -v rd="$remaining_dur" -v d="$dur_i" -v sa="$sum_actual" -v sr="$sum_req" -v last="$((i == n - 1))" 'BEGIN {
            ratio = (sr > 0) ? sa / sr : 1
            lowest = last ? 1 : 0.85
            if (ratio < lowest) ratio = lowest
            if (ratio > 1.6) ratio = 1.6
            bps = (rb * d / rd) * 8 / d / ratio
            if (bps < 50000) bps = 50000
            printf "%.0f %.0f", bps, bps * d / 8
        }')

        if ! ffmpeg -y -hide_banner -loglevel error -stats "${pre_chunk[@]}" -ss "$start" "${limit[@]}" -i "$file" \
                -vf "$vf" "${ENC[@]}" \
                -b:v "$target_bps" -maxrate "$((target_bps * rc_max / 100))" -bufsize "$((target_bps * rc_buf / 100))" \
                -an -f matroska "$work/chunk_$i.mkv"; then
            rm -rf "$work"
            return 1
        fi

        actual=$(stat -c %s "$work/chunk_$i.mkv")
        ratio_note=$(LC_ALL=C awk -v a="$actual" -v r="$req_bytes" 'BEGIN { printf "x%.2f", a / r }')
        echo "   chunk $((i + 1))/$n: ${actual} B (${ratio_note} of requested)"
        sum_actual=$((sum_actual + actual))
        sum_req=$((sum_req + req_bytes))
        remaining_bytes=$((remaining_bytes - actual))
        remaining_dur=$(LC_ALL=C awk -v r="$remaining_dur" -v d="$dur_i" 'BEGIN { printf "%.6f", r - d }')
        printf "file '%s'\n" "$work/chunk_$i.mkv" >> "$list"
    done

    # Join without re-encoding; the HEVC tag of the encoder settings is kept
    for ((idx = 0; idx < ${#ENC[@]} - 1; idx++)); do
        if [ "${ENC[idx]}" = "-tag:v" ]; then
            tag_args=(-tag:v "${ENC[idx + 1]}")
        fi
    done
    if ! ffmpeg -y -hide_banner -loglevel error -f concat -safe 0 -i "$list" "${audio_in[@]}" \
            -map 0:v "${audio_map[@]}" -c copy "${tag_args[@]}" "$OUTPUT"; then
        rm -rf "$work"
        return 1
    fi
    rm -rf "$work"
}

[ ${#FILES[@]} -gt 0 ] || exit 0

if ! [[ "$BUDGET_MB" =~ ^[0-9]+([.][0-9]+)?$ ]] || ! awk -v v="$BUDGET_MB" 'BEGIN { exit !(v > 0) }'; then
    die "$MSG_INVALID_LIMIT $BUDGET_MB"
fi

case "$CODEC_KEY" in
    h264_cpu)
        LABEL="H.264 [CPU]"
        PRE=()
        ENC=(-c:v libx264 -profile:v main -pix_fmt yuv420p -preset medium)
        RC_MAX=200; RC_BUF=600; AIM=99; BPP_MIN=0.07
        SUFFIX="h264cpu"
        ;;
    h264_nv)
        LABEL="H.264 [GPU]"
        PRE=(-hwaccel cuda -hwaccel_output_format cuda)
        ENC=(-c:v h264_nvenc -profile:v main -preset p7 -rc:v vbr)
        RC_MAX=145; RC_BUF=200; AIM=98; BPP_MIN=0.07
        SUFFIX="h264gpu"
        ;;
    h265_cpu)
        LABEL="HEVC [CPU]"
        PRE=()
        ENC=(-c:v libx265 -profile:v main -pix_fmt yuv420p -preset medium -tag:v hvc1)
        RC_MAX=200; RC_BUF=600; AIM=98.5; BPP_MIN=0.05
        SUFFIX="hevccpu"
        ;;
    h265_nv)
        LABEL="HEVC [GPU]"
        PRE=(-hwaccel cuda -hwaccel_output_format cuda)
        ENC=(-c:v hevc_nvenc -profile:v main -preset p7 -rc:v vbr -tag:v hvc1)
        RC_MAX=145; RC_BUF=200; AIM=98; BPP_MIN=0.05
        SUFFIX="hevcgpu"
        ;;
    av1_cpu)
        LABEL="AV1 [CPU]"
        PRE=()
        ENC=(-c:v libsvtav1 -preset 6 -pix_fmt yuv420p)
        RC_MAX=145; RC_BUF=200; AIM=96; BPP_MIN=0.05
        SUFFIX="av1cpu"
        ;;
    av1_nv)
        LABEL="AV1 [GPU]"
        PRE=(-hwaccel cuda -hwaccel_output_format cuda)
        ENC=(-c:v av1_nvenc -preset p7 -rc:v vbr)
        RC_MAX=145; RC_BUF=200; AIM=98; BPP_MIN=0.05
        SUFFIX="av1gpu"
        ;;
    h264_amd|h264_intel)
        vaapi_setup "${CODEC_KEY##*_}"
        LABEL="H.264 [GPU] $GPU_NAME"
        ENC=(-c:v h264_vaapi -profile:v main)
        SUFFIX="h264${CODEC_KEY##*_}"
        RC_MAX=145; RC_BUF=200; AIM=98; BPP_MIN=0.07
        ;;
    h265_amd|h265_intel)
        vaapi_setup "${CODEC_KEY##*_}"
        LABEL="HEVC [GPU] $GPU_NAME"
        ENC=(-c:v hevc_vaapi -profile:v main -tag:v hvc1)
        SUFFIX="hevc${CODEC_KEY##*_}"
        RC_MAX=145; RC_BUF=200; AIM=98; BPP_MIN=0.05
        ;;
    av1_amd|av1_intel)
        vaapi_setup "${CODEC_KEY##*_}"
        LABEL="AV1 [GPU] $GPU_NAME"
        ENC=(-c:v av1_vaapi)
        SUFFIX="av1${CODEC_KEY##*_}"
        RC_MAX=145; RC_BUF=200; AIM=98; BPP_MIN=0.05
        ;;
    discord)
        LABEL="Discord (HEVC [CPU])"
        PRE=()
        ENC=(-c:v libx265 -profile:v main -pix_fmt yuv420p -preset medium -tag:v hvc1)
        RC_MAX=200; RC_BUF=600; AIM=98.5; BPP_MIN=0.05
        SUFFIX="discord"
        ;;
    *)
        die "$MSG_UNKNOWN_CODEC $CODEC_KEY"
        ;;
esac

echo "=== $MSG_HEADER $LABEL | $MSG_BUDGET ${BUDGET_MB} MB ==="
echo

FAILED=0
for file in "${FILES[@]}"; do
    [ -f "$file" ] || continue

    # --- Probe the source. The budget math below needs the exact duration, the
    # frame rate (container overhead grows with the number of frames) and
    # whether there is an audio track at all (no track = no audio bitrate to reserve).
    DURATION=$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null | head -n1)
    if ! awk -v d="$DURATION" 'BEGIN { exit !(d + 0 > 0) }'; then
        DURATION=$(ffprobe -v error -select_streams v:0 -show_entries stream=duration -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null | head -n1)
        awk -v d="$DURATION" 'BEGIN { exit !(d + 0 > 0) }' || DURATION=1
    fi
    FPS=$(ffprobe -v error -select_streams v:0 -show_entries stream=avg_frame_rate -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null | head -n1 \
          | awk -F/ '{ if ($2 + 0 > 0 && $1 + 0 > 0) printf "%.3f", $1 / $2; else print 30 }')
    [ -n "$FPS" ] || FPS=30
    if [ -n "$(ffprobe -v error -select_streams a:0 -show_entries stream=codec_type -of csv=p=0 "$file" 2>/dev/null)" ]; then
        HAS_AUDIO=1
    else
        HAS_AUDIO=0
    fi

    # Two-pass mode exists for the CPU H.264 / HEVC encoders only
    USE_2PASS=0
    if [ "${MMT_TWOPASS:-0}" = "1" ]; then
        case "$CODEC_KEY" in
            h264_cpu|h265_cpu|discord) USE_2PASS=1 ;;
        esac
    fi

    # Hardware encoders (NVENC / VAAPI) are encoded in chunks with feedback, see
    # encode_chunked. MMT_CHUNKED=1 / 0 forces it on / off for any codec (for testing).
    USE_CHUNKS=0
    case "$CODEC_KEY" in
        *_nv|*_amd|*_intel) USE_CHUNKS=1 ;;
    esac
    case "${MMT_CHUNKED:-}" in
        1) USE_CHUNKS=1 ;;
        0) USE_CHUNKS=0 ;;
    esac
    # Two-pass and chunked encoding see the real result, so neither needs the extra
    # reserve for short videos below; chunked mode replaces two-pass
    AIM_EFF="$AIM"
    if [ "$USE_CHUNKS" -eq 1 ]; then
        USE_2PASS=0
        AIM_EFF=99
    fi
    EXACT=$(( USE_2PASS || USE_CHUNKS ))

    # --- Bitrates (bit/s), computed from the real budget in bytes:
    #   * AIM% of the limit is the target (a small reserve against rate-control error;
    #     it is codec specific, see the table above). One-pass rate control needs a
    #     moment to settle, so its relative error grows on short videos: the reserve
    #     gets an extra 30/duration percent (capped at 8%). Two-pass sees the whole video
    #     first and does not need that extra reserve.
    #   * the MP4 container overhead is estimated from the frame counts
    #     (measured: ~4 B per video frame, ~17 B per AAC frame, ~1 KB fixed; rounded up)
    #   * the audio bitrate scales down for very tight budgets and is 0 without audio
    read -r VIDEO_BPS AUDIO_BPS TIGHT < <(awk -v mb="$BUDGET_MB" -v dur="$DURATION" -v fps="$FPS" -v aud="$HAS_AUDIO" -v aim="$AIM_EFF" -v twopass="$EXACT" 'BEGIN {
        extra    = twopass ? 0 : 30 / dur; if (extra > 8) extra = 8
        budget   = mb * 1048576 * (aim - extra) / 100
        overhead = 4096 + dur * fps * 12 + (aud ? dur * 47 * 20 : 0)
        total    = (budget - overhead) * 8 / dur
        if (!aud)                a = 0
        else if (total > 400000) a = 128000
        else if (total > 150000) a = 96000
        else                     a = 64000
        v = total - a; tight = 0
        if (v < 50000) { v = 50000; tight = 1 }
        printf "%d %d %d", v, a, tight
    }')

    if [ "$TIGHT" -eq 1 ]; then
        echo "[!] \"$(basename "$file")\": $MSG_TIGHT"
    fi

    # --- Resolution. The scale factor is relative to the SHORTER side, so portrait
    # videos behave like landscape ones; nothing is ever scaled up. "auto" walks down
    # a ladder of sizes and takes the largest one that still gets BPP_MIN bits per
    # pixel at the bitrate computed above (frame rates above 30 count only half,
    # because fast video has more in common between frames).
    SCALE_F=1
    SCALE_INFO=""
    VF_ARGS=()
    SCALE_FILTER=""
    PRE_EFF=("${PRE[@]}")
    # (width and height are asked separately: for rotated videos ffprobe adds a stray
    # separator to a combined CSV line)
    SRC_W=$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null | head -n1)
    SRC_H=$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null | head -n1)
    if [ "$SCALE_MODE" != "orig" ] && [[ "$SRC_W" =~ ^[0-9]+$ ]] && [[ "$SRC_H" =~ ^[0-9]+$ ]]; then
        read -r SCALE_F SCALE_SHORT SCALE_BPP < <(LC_ALL=C awk -v mode="$SCALE_MODE" -v w="$SRC_W" -v h="$SRC_H" -v fps="$FPS" -v vbps="$VIDEO_BPS" -v t="$BPP_MIN" 'BEGIN {
            short = (w < h) ? w : h
            fe = (fps > 30) ? 30 + (fps - 30) * 0.5 : fps
            f = 1
            if (mode ~ /^s/)      f = substr(mode, 2) + 0
            else if (mode ~ /^h/) f = (substr(mode, 2) + 0) / short
            else if (mode == "auto" && vbps / (w * h * fe) < t) {
                n = split("2160 1440 1080 720 540 480 360 240", ladder, " ")
                f = 0
                for (i = 1; i <= n; i++) {
                    if (ladder[i] >= short) continue
                    cf = ladder[i] / short
                    if (vbps / (w * h * cf * cf * fe) >= t) { f = cf; break }
                }
                if (f == 0) f = (short > 240) ? 240 / short : 1   # nothing is good enough: smallest step
            }
            if (f > 1 || f >= 0.999) f = 1
            printf "%.6f %d %.3f", f, 2 * int(short * f / 2 + 0.5), vbps / (w * h * f * f * fe)
        }')
        if LC_ALL=C awk -v f="$SCALE_F" 'BEGIN { exit !(f < 1) }'; then
            SCALE_FILTER="scale=2*round(iw*${SCALE_F}/2):2*round(ih*${SCALE_F}/2):flags=lanczos"
            SCALE_INFO=" | scale x${SCALE_F} (shorter side ${SCALE_SHORT}px, ${SCALE_BPP} bit/px)"
            # GPU frames cannot go through the software scaler: decode on the GPU, scale in memory
            case "$CODEC_KEY" in
                *_nv) PRE_EFF=(-hwaccel cuda) ;;
            esac
        fi
    fi

    # Filter chain: software scaling first, then (VAAPI only) upload the frames to the GPU
    VIDEO_FILTERS="$SCALE_FILTER"
    if [ "$USE_VAAPI" -eq 1 ]; then
        VIDEO_FILTERS="${VIDEO_FILTERS:+$VIDEO_FILTERS,}format=nv12,hwupload"
    fi
    if [ -n "$VIDEO_FILTERS" ]; then
        VF_ARGS=(-vf "$VIDEO_FILTERS")
    fi

    MAXRATE_BPS=$(( VIDEO_BPS * RC_MAX / 100 ))
    BUFSIZE_BPS=$(( VIDEO_BPS * RC_BUF / 100 ))

    if [ "$HAS_AUDIO" -eq 1 ]; then
        AUDIO_ARGS=(-c:a aac -b:a "$AUDIO_BPS")
    else
        AUDIO_ARGS=()
    fi

    if [ "$CODEC_KEY" = "discord" ]; then
        OUTPUT="${file%.*}_discord.mp4"
    else
        OUTPUT="${file%.*}_limit_${BUDGET_MB}MB_${SUFFIX}.mp4"
    fi

    # Optional two-pass mode (CPU H.264 / HEVC only): pass 1 only analyses the video and
    # writes no output, pass 2 makes the single real encode with the whole video known
    # in advance, which is what makes the final size predictable.
    PASS_ARGS=()
    PASS1_ARGS=()
    PASSDIR=""
    TWOPASS_NOTE=""
    if [ "$USE_2PASS" -eq 1 ]; then
        PASSDIR=$(mktemp -d)
        if [ "$CODEC_KEY" = "h264_cpu" ]; then
            PASS1_ARGS=(-pass 1 -passlogfile "$PASSDIR/pass")
            PASS_ARGS=(-pass 2 -passlogfile "$PASSDIR/pass")
        else
            # x265 takes its pass settings through -x265-params
            PASS1_ARGS=(-x265-params "pass=1:stats=$PASSDIR/x265.log")
            PASS_ARGS=(-x265-params "pass=2:stats=$PASSDIR/x265.log")
        fi
        TWOPASS_NOTE=" | 2-pass"
    fi
    if [ "$USE_CHUNKS" -eq 1 ]; then
        TWOPASS_NOTE=" | chunked"
    fi

    echo "-> $(basename "$file") | duration=${DURATION}s | video=$((VIDEO_BPS / 1000))k audio=$((AUDIO_BPS / 1000))k${TWOPASS_NOTE}${SCALE_INFO} -> $(basename "$OUTPUT")"

    if [ "$USE_CHUNKS" -eq 1 ]; then
        if ! encode_chunked; then
            echo
            echo "[!] $MSG_FAILED $file"
            FAILED=1
            continue
        fi
    else
        if [ -n "$PASSDIR" ]; then
            if ! ffmpeg -y "${PRE_EFF[@]}" -i "$file" "${VF_ARGS[@]}" "${ENC[@]}" \
                    -b:v "$VIDEO_BPS" -maxrate "$MAXRATE_BPS" -bufsize "$BUFSIZE_BPS" \
                    "${PASS1_ARGS[@]}" -an -f null /dev/null; then
                echo
                echo "[!] $MSG_FAILED $file"
                FAILED=1
                rm -rf "$PASSDIR"
                continue
            fi
        fi

        # Single encoding pass: bitrate is pre-computed above, no second/duplicate encode.
        if ! ffmpeg -y "${PRE_EFF[@]}" -i "$file" "${VF_ARGS[@]}" "${ENC[@]}" \
                -b:v "$VIDEO_BPS" -maxrate "$MAXRATE_BPS" -bufsize "$BUFSIZE_BPS" \
                "${PASS_ARGS[@]}" "${AUDIO_ARGS[@]}" \
                "$OUTPUT"; then
            echo
            echo "[!] $MSG_FAILED $file"
            FAILED=1
            [ -z "$PASSDIR" ] || rm -rf "$PASSDIR"
            continue
        fi
        [ -z "$PASSDIR" ] || rm -rf "$PASSDIR"
    fi

    if [ -f "$OUTPUT" ]; then
        # Exact size in bytes (du would round up to whole MB and count disk blocks)
        ACTUAL_BYTES=$(stat -c %s "$OUTPUT")
        awk -v b="$ACTUAL_BYTES" -v mb="$BUDGET_MB" -v done="$MSG_DONE" -v target="$MSG_TARGET" 'BEGIN {
            printf "   %s %.2f MB (%.2f MB in decimal units) | %s %s MB = %.1f%%\n", done, b / 1048576, b / 1000000, target, mb, b / (mb * 1048576) * 100
        }'
        if awk -v b="$ACTUAL_BYTES" -v mb="$BUDGET_MB" 'BEGIN { exit !(b > mb * 1048576) }'; then
            awk -v b="$ACTUAL_BYTES" -v mb="$BUDGET_MB" -v over="$MSG_OVER" 'BEGIN { printf "[!] %s %.2f MB\n", over, b / 1048576 - mb }'
        fi
    fi
    echo
done

# Keep the window open only when ffmpeg failed, otherwise the error would vanish
# instantly. A successful run closes the window by itself.
if [ "$FAILED" -ne 0 ]; then
    echo
    read -r -p "$MSG_PRESS_ENTER"
    exit 1
fi
