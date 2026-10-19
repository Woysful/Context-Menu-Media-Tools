#!/bin/bash

# KDE Plasma 6 Context Menu Installer for Multimedia Files
# Based on Windows Context Menu Registry entries
# Author: Adapted for Linux KDE Plasma 6

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Installation directory
# kio/servicemenus is correct for KDE Plasma 6 Service Menus
INSTALL_DIR="$HOME/.local/share/kio/servicemenus"

# Directory for helper scripts used by service menu actions
# (terminal launcher, dialog, GPU helper, Compress Video, Convert Video / Audio,
# Extract Audio, Make frame sequence, Convert to WebP anim)
SCRIPTS_DIR="$HOME/.local/share/multimedia-context-menu-scripts"

# Helper scripts live in the repository under linux/scripts/.
# The installer uses a local copy when it is run from a cloned repository and
# downloads the files from GitHub otherwise.
#   MMT_BRANCH        - branch/tag to download from (default: main)
#   MMT_REPO_RAW_URL  - override the raw files base URL (forks, mirrors, testing)
REPO_RAW_URL="${MMT_REPO_RAW_URL:-https://raw.githubusercontent.com/Woysful/Context-Menu-Media-Tools/${MMT_BRANCH:-main}}"
HELPER_SCRIPTS_PATH="linux/scripts"
HELPER_SCRIPTS=(mmt-terminal mmt-dialog mmt-gpu.sh compress_video.sh compress_mp4.sh compress_limit.sh convert_media.sh convert_webp_anim.sh)

# Directory of this installer (empty when the script is piped into bash)
SOURCE_DIR=""
if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
    SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi

# Function to detect user language
detect_language() {
    local lang="${LANG:-${LC_MESSAGES:-en}}"
    case "${lang%%_*}" in
        ru|be|kk) echo "ru" ;;
        uk) echo "uk" ;;
        *) echo "en" ;;
    esac
}

# Get user language
USER_LANG=$(detect_language)

# Localized messages
case "$USER_LANG" in
    ru)
        MSG_TITLE="KDE Plasma 6 Multimedia Context Menu Installer"
        MSG_CHECKING_DEPS="Проверка зависимостей..."
        MSG_DEPS_FOUND="✓ Все зависимости найдены"
        MSG_MISSING_DEPS="Отсутствуют зависимости:"
        MSG_INSTALL_DEPS="Установите их с помощью менеджера пакетов:"
        MSG_CREATING_AUDIO="Создание меню аудио..."
        MSG_AUDIO_CREATED="✓ Меню аудио создано"
        MSG_CREATING_IMAGE="Создание меню изображений..."
        MSG_IMAGE_CREATED="✓ Меню изображений создано"
        MSG_CREATING_VIDEO="Создание меню видео..."
        MSG_VIDEO_CREATED="✓ Меню видео создано"
        MSG_INSTALLING_SCRIPTS="Установка вспомогательных скриптов..."
        MSG_SCRIPTS_READY="✓ Вспомогательные скрипты установлены"
        MSG_COPYING="копирование"
        MSG_DOWNLOADING="загрузка"
        MSG_DOWNLOAD_FAILED="Не удалось загрузить:"
        MSG_BAD_SCRIPT="Получен некорректный файл (ожидался shell-скрипт):"
        MSG_NEED_DOWNLOADER="Для загрузки скриптов нужен curl или wget"
        MSG_CONVERT_AUDIO="Конвертировать аудио"
        MSG_SETUP_DIR="Настройка директории установки..."
        MSG_DIR_READY="✓ Директория настроена"
        MSG_INSTALL_COMPLETE="Установка завершена!"
        MSG_RESTART_DOLPHIN="Перезапустите Dolphin или выйдите из системы и\nвойдите снова, чтобы изменения вступили в силу."
        MSG_UNINSTALL_TITLE="Удаление KDE Plasma 6 Multimedia Context Menu"
        MSG_REMOVING_FILES="Удаление файлов контекстного меню..."
        MSG_FILES_REMOVED="✓ Файлы контекстного меню удалены"
        MSG_DIR_NOT_FOUND="Директория установки не найдена:"
        MSG_UNINSTALL_COMPLETE="Удаление завершено!"
        MSG_RESTART_AFTER_UNINSTALL="Перезапустите Dolphin или выйдите из системы и войдите снова."
        ;;
    uk)
        MSG_TITLE="Встановлювач контекстного меню мультимедіа KDE Plasma 6"
        MSG_CHECKING_DEPS="Перевірка залежностей..."
        MSG_DEPS_FOUND="✓ Усі залежності знайдені"
        MSG_MISSING_DEPS="Відсутні залежності:"
        MSG_INSTALL_DEPS="Встановіть їх за допомогою менеджера пакунків:"
        MSG_CREATING_AUDIO="Створення меню аудіо..."
        MSG_AUDIO_CREATED="✓ Меню аудіо створено"
        MSG_CREATING_IMAGE="Створення меню зображень..."
        MSG_IMAGE_CREATED="✓ Меню зображень створено"
        MSG_CREATING_VIDEO="Створення меню відео..."
        MSG_VIDEO_CREATED="✓ Меню відео створено"
        MSG_INSTALLING_SCRIPTS="Встановлення допоміжних скриптів..."
        MSG_SCRIPTS_READY="✓ Допоміжні скрипти встановлено"
        MSG_COPYING="копіювання"
        MSG_DOWNLOADING="завантаження"
        MSG_DOWNLOAD_FAILED="Не вдалося завантажити:"
        MSG_BAD_SCRIPT="Отримано некоректний файл (очікувався shell-скрипт):"
        MSG_NEED_DOWNLOADER="Для завантаження скриптів потрібен curl або wget"
        MSG_CONVERT_AUDIO="Конвертувати аудіо"
        MSG_SETUP_DIR="Налаштування директорії встановлення..."
        MSG_DIR_READY="✓ Директорія налаштована"
        MSG_INSTALL_COMPLETE="Встановлення завершено!"
        MSG_RESTART_DOLPHIN="Перезапустіть Dolphin або вийдіть із системи та\nувійдіть знову, щоб зміни набрали чинності."
        MSG_UNINSTALL_TITLE="Видалення контекстного меню мультимедіа KDE Plasma 6"
        MSG_REMOVING_FILES="Видалення файлів контекстного меню..."
        MSG_FILES_REMOVED="✓ Файли контекстного меню видалені"
        MSG_DIR_NOT_FOUND="Директорія встановлення не знайдена:"
        MSG_UNINSTALL_COMPLETE="Видалення завершено!"
        MSG_RESTART_AFTER_UNINSTALL="Перезапустіть Dolphin або вийдіть із системи та увійдіть знову."
        ;;
    *)
        MSG_TITLE="KDE Plasma 6 Multimedia Context Menu Installer"
        MSG_CHECKING_DEPS="Checking dependencies..."
        MSG_DEPS_FOUND="✓ All dependencies found"
        MSG_MISSING_DEPS="Missing dependencies:"
        MSG_INSTALL_DEPS="Please install them using your package manager:"
        MSG_CREATING_AUDIO="Creating audio menu..."
        MSG_AUDIO_CREATED="✓ Audio menu created"
        MSG_CREATING_IMAGE="Creating image menu..."
        MSG_IMAGE_CREATED="✓ Image menu created"
        MSG_CREATING_VIDEO="Creating video menu..."
        MSG_VIDEO_CREATED="✓ Video menu created"
        MSG_INSTALLING_SCRIPTS="Installing helper scripts..."
        MSG_SCRIPTS_READY="✓ Helper scripts installed"
        MSG_COPYING="copying"
        MSG_DOWNLOADING="downloading"
        MSG_DOWNLOAD_FAILED="Failed to download:"
        MSG_BAD_SCRIPT="Received an unexpected file (a shell script was expected):"
        MSG_NEED_DOWNLOADER="curl or wget is required to download the helper scripts"
        MSG_CONVERT_AUDIO="Convert Audio"
        MSG_SETUP_DIR="Setting up installation directory..."
        MSG_DIR_READY="✓ Directory configured"
        MSG_INSTALL_COMPLETE="Installation completed!"
        MSG_RESTART_DOLPHIN="Please restart Dolphin or log out and back in\nfor changes to take effect."
        MSG_UNINSTALL_TITLE="Removing KDE Plasma 6 Multimedia Context Menu"
        MSG_REMOVING_FILES="Removing multimedia context menu files..."
        MSG_FILES_REMOVED="✓ Context menu files removed"
        MSG_DIR_NOT_FOUND="Installation directory not found:"
        MSG_UNINSTALL_COMPLETE="Uninstallation completed!"
        MSG_RESTART_AFTER_UNINSTALL="Please restart Dolphin or log out and back in for changes to take effect."
        ;;
esac

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to print step progress
print_step() {
    local step="$1"
    local total="$2"
    local message="$3"
    echo -e "${BLUE}[${step}/${total}]${NC} ${message}"
}

# Function to print section header
print_header() {
    local title="${1:-$MSG_TITLE}"
    echo
    echo -e "${BLUE}─────────────────────────────────────────────────────────${NC}"
    echo -e "${BLUE}     $title${NC}"
    echo -e "${BLUE}─────────────────────────────────────────────────────────${NC}"
    echo
}

# Function to print completion banner
print_completion() {
    echo
    echo -e "${GREEN}─────────────────────────────────────────────────────────${NC}"
    echo -e "${GREEN}                ${MSG_INSTALL_COMPLETE}                  ${NC}"
    echo -e "${GREEN}─────────────────────────────────────────────────────────${NC}"
    echo
    echo -e "${MSG_RESTART_DOLPHIN}"
    echo
    echo repo: https://github.com/Woysful/Context-Menu-Media-Tools
}

# Function to check dependencies
check_dependencies() {
    print_step "1" "6" "$MSG_CHECKING_DEPS"

    local deps=("ffmpeg" "ffprobe" "python3")
    local missing_deps=()

    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            missing_deps+=("$dep")
        fi
    done

    # The dialogs are drawn with Qt: PyQt6 or PySide6 is needed
    if command -v python3 &> /dev/null \
        && ! python3 -c 'import PyQt6.QtWidgets' &> /dev/null \
        && ! python3 -c 'import PySide6.QtWidgets' &> /dev/null; then
        missing_deps+=("python-pyqt6 (or python-pyside6)")
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        print_error "$MSG_MISSING_DEPS ${missing_deps[*]}"
        print_info "$MSG_INSTALL_DEPS"
        print_info "  Arch Linux: sudo pacman -S ffmpeg python-pyqt6"
        print_info "  Ubuntu/Debian: sudo apt install ffmpeg python3-pyqt6"
        exit 1
    fi

    print_success "$MSG_DEPS_FOUND"
}


# Function to create audio context menu
create_audio_menu() {
    print_step "4" "6" "$MSG_CREATING_AUDIO"

    # Audio Convert Menu
    cat > "$INSTALL_DIR/multimedia-convert-audio.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=audio/wav;audio/mpeg;audio/flac;audio/aac;audio/ogg;audio/x-ms-wma;audio/mp4;audio/webm;audio/opus
Actions=convertWAV;convertMP3;convertFLAC;convertAAC;convertOGG;convertWMA;convertM4A;convertOpus;statsAudio
X-KDE-Submenu=Convert Audio
X-KDE-Submenu[ru]=Конвертировать
X-KDE-Submenu[uk]=Конвертувати
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action convertWAV]
Name=WAV
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a pcm_s16le "${file%.*}.wav"; done' bash %F
Icon=audio-wav

[Desktop Action convertMP3]
Name=MP3
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -ab 192k -map_metadata 0 -id3v2_version 3 "${file%.*}.mp3"; done' bash %F
Icon=audio-mpeg

[Desktop Action convertFLAC]
Name=FLAC
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a flac "${file%.*}.flac"; done' bash %F
Icon=audio-flac

[Desktop Action convertAAC]
Name=AAC
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a aac -b:a 192k "${file%.*}.aac"; done' bash %F
Icon=audio-aac

[Desktop Action convertOGG]
Name=OGG
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a libvorbis -q:a 0 "${file%.*}.ogg"; done' bash %F
Icon=audio-ogg

[Desktop Action convertWMA]
Name=WMA
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a wmav2 -b:a 192k "${file%.*}.wma"; done' bash %F
Icon=audio-x-ms-wma

[Desktop Action convertM4A]
Name=M4A
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a aac -b:a 192k "${file%.*}.m4a"; done' bash %F
Icon=audio-mp4

[Desktop Action convertOpus]
Name=Opus
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -q:a 0 -c:a libopus "${file%.*}.opus"; done' bash %F
Icon=audio-opus

[Desktop Action statsAudio]
Name=Stats
Name[ru]=Детали
Name[uk]=Деталі
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffprobe -hide_banner -i "$file"; echo "Press Enter to continue..."; read; done' bash %F
Icon=documentinfo
EOF

    print_success "$MSG_AUDIO_CREATED"
}

# Function to create image context menu
create_image_menu() {
    print_step "5" "6" "$MSG_CREATING_IMAGE"

    # Image Convert Menu
    cat > "$INSTALL_DIR/multimedia-convert-image.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=image/jpeg;image/png;image/bmp;image/tiff;image/gif;image/webp;image/x-tga;image/vnd.adobe.photoshop
Actions=convertJPG;convertPNG;convertBMP;convertTIFF;convertGIF;convertWebP;convertTGA;statsImage
X-KDE-Submenu=Convert Image
X-KDE-Submenu[ru]=Конвертировать
X-KDE-Submenu[uk]=Конвертувати
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action convertJPG]
Name=JPG
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -q:v 5 "${file%.*}.jpg"; done' bash %F
Icon=image-jpeg

[Desktop Action convertPNG]
Name=PNG
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" "${file%.*}.png"; done' bash %F
Icon=image-png

[Desktop Action convertBMP]
Name=BMP
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" "${file%.*}.bmp"; done' bash %F
Icon=image-bmp

[Desktop Action convertTIFF]
Name=TIFF
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" "${file%.*}.tiff"; done' bash %F
Icon=image-tiff

[Desktop Action convertGIF]
Name=GIF
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -f gif "${file%.*}.gif"; done' bash %F
Icon=image-gif

[Desktop Action convertWebP]
Name=WebP
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libwebp -q:v 95 "${file%.*}.webp"; done' bash %F
Icon=image-webp

[Desktop Action convertTGA]
Name=TGA
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" "${file%.*}.tga"; done' bash %F
Icon=image-x-tga

[Desktop Action statsImage]
Name=Stats
Name[ru]=Детали
Name[uk]=Деталі
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffprobe -hide_banner -i "$file"; echo "Press Enter to continue..."; read; done' bash %F
Icon=documentinfo
EOF

    print_success "$MSG_IMAGE_CREATED"
}

# Function to create video context menu
create_video_menu() {
    print_step "6" "6" "$MSG_CREATING_VIDEO"

    # The video items are plain (top-level) actions. Dolphin lists plain actions
    # sorted by their action identifier (the name in Actions=), so the numeric
    # prefix ("01_", "02_", ...) defines the order of the menu:
    #   Compress Video, Convert Video, Convert Audio,
    #   Extract Audio, Delete sound, Make frame sequence, Convert to WebP anim, Stats
    # (Submenus are always listed before plain actions, which is why every
    # item is a plain action with its own dialog instead of a submenu.)

    # Compress Video: one item, one dialog with two tabs - "Presets" (compression level +
    # codec, compress_mp4.sh) and "Size limit" (encode to a target file size,
    # compress_limit.sh). compress_video.sh shows the dialog and starts the chosen one.
    cat > "$INSTALL_DIR/multimedia-compress-video.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=01_compressVideo
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action 01_compressVideo]
Name=Compress Video
Name[ru]=Сжать видео
Name[uk]=Стиснути відео
Exec=@SCRIPTS_DIR@/compress_video.sh %F
Icon=video-compress
EOF

    # Convert Video: the output format is chosen in a dialog
    cat > "$INSTALL_DIR/multimedia-convert-video.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=02_convertVideo
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action 02_convertVideo]
Name=Convert Video
Name[ru]=Конвертировать видео
Name[uk]=Конвертувати відео
Exec=@SCRIPTS_DIR@/convert_media.sh video %F
Icon=video-mp4
EOF

    # Convert Audio (audio tracks of a video): the output format is chosen in a dialog
    cat > "$INSTALL_DIR/multimedia-convert-video-audio.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=03_convertAudio
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action 03_convertAudio]
Name=Convert Audio
Name[ru]=Конвертировать аудио
Name[uk]=Конвертувати аудіо
Exec=@SCRIPTS_DIR@/convert_media.sh audio %F
Icon=audio-wav
EOF

    # Extract Audio: the output format is chosen in a dialog
    cat > "$INSTALL_DIR/multimedia-extract-audio.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=04_extractAudio
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action 04_extractAudio]
Name=Extract Audio
Name[ru]=Извлечь аудио
Name[uk]=Витягнути аудіо
Exec=@SCRIPTS_DIR@/convert_media.sh extract %F
Icon=audio-mpeg
EOF

    # Delete Sound Menu
    cat > "$INSTALL_DIR/multimedia-delete-sound.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=05_deleteSound
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action 05_deleteSound]
Name=Delete sound
Name[ru]=Удалить звук
Name[uk]=Видалити звук
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffmpeg -i "$file" -an -c copy "${file%.*} (no audio).${file##*.}"; done' bash %F
Icon=media-track-remove-amarok
EOF

    # Make Frame Sequence: the image format is chosen in a dialog
    cat > "$INSTALL_DIR/multimedia-make-frame-sequence.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=06_makeFrames
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action 06_makeFrames]
Name=Make frame sequence
Name[ru]=Разбить на кадры
Name[uk]=Розбити на кадри
Exec=@SCRIPTS_DIR@/convert_media.sh frames %F
Icon=image-png
EOF

    # Convert to animated WebP: size limit, FPS, quality and compression are chosen in one dialog
    cat > "$INSTALL_DIR/multimedia-convert-webp-anim.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=07_convertWebPAnim
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action 07_convertWebPAnim]
Name=Convert to WebP anim
Name[ru]=Конвертировать в анимированный WebP
Name[uk]=Конвертувати в анімований WebP
Exec=@SCRIPTS_DIR@/convert_webp_anim.sh %F
Icon=image-webp
EOF

    # Stats Menu
    cat > "$INSTALL_DIR/multimedia-stats.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=audio/wav;audio/mpeg;audio/flac;audio/aac;audio/ogg;audio/x-ms-wma;audio/mp4;audio/webm;audio/opus;image/jpeg;image/png;image/bmp;image/tiff;image/gif;image/webp;image/x-tga;image/vnd.adobe.photoshop;video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=08_stats
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action 08_stats]
Name=Stats
Name[ru]=Детали
Name[uk]=Деталі
Exec=@TERMINAL@ bash -c 'for file in "$@" ; do ffprobe -hide_banner -i "$file"; echo "Press Enter to continue..."; read; done' bash %F
Icon=documentinfo
EOF

    print_success "$MSG_VIDEO_CREATED"
}

# Download a URL into a file using curl or wget
download_file() {
    local url="$1" dest="$2"
    if command -v curl &> /dev/null; then
        curl -fsSL --retry 2 -o "$dest" "$url"
    elif command -v wget &> /dev/null; then
        wget -q -O "$dest" "$url"
    else
        print_error "$MSG_NEED_DOWNLOADER"
        return 2
    fi
}

# Function to install helper scripts (local copy if available, GitHub otherwise)
install_helper_scripts() {
    print_step "3" "6" "$MSG_INSTALLING_SCRIPTS"

    mkdir -p "$SCRIPTS_DIR"

    local name tmp local_path url
    for name in "${HELPER_SCRIPTS[@]}"; do
        tmp="$(mktemp)"
        local_path="$SOURCE_DIR/$HELPER_SCRIPTS_PATH/$name"
        url="$REPO_RAW_URL/$HELPER_SCRIPTS_PATH/$name"

        if [ -n "$SOURCE_DIR" ] && [ -f "$local_path" ]; then
            print_info "  $MSG_COPYING $name"
            cp "$local_path" "$tmp"
        else
            print_info "  $MSG_DOWNLOADING $name"
            if ! download_file "$url" "$tmp"; then
                rm -f "$tmp"
                print_error "$MSG_DOWNLOAD_FAILED $url"
                exit 1
            fi
        fi

        # Sanity check: must be a script (guards against HTML error pages etc.)
        if [ "$(head -c 2 "$tmp")" != "#!" ]; then
            rm -f "$tmp"
            print_error "$MSG_BAD_SCRIPT $name"
            exit 1
        fi

        # (not "install": that name is taken by this script's own install() function)
        cp "$tmp" "$SCRIPTS_DIR/$name"
        chmod 755 "$SCRIPTS_DIR/$name"
        rm -f "$tmp"
    done

    print_success "$MSG_SCRIPTS_READY"
}

# Remove menu files left over from older versions
remove_legacy_files() {
    # "Compress to MP4+" / "MP4++" were merged into "Compress to MP4"
    rm -f "$INSTALL_DIR/multimedia-compress-mp4-plus.desktop"
    rm -f "$INSTALL_DIR/multimedia-compress-mp4-plus-plus.desktop"
    # "Compress to MP4" and "Compress to Limit" were merged into "Compress Video"
    rm -f "$INSTALL_DIR/multimedia-compress-mp4.desktop"
    rm -f "$INSTALL_DIR/multimedia-compress-limit.desktop"
}

# Fill in install-time paths and set permissions on the generated .desktop files
finalize_desktop_files() {
    local dir_escaped
    dir_escaped="$(printf '%s' "$SCRIPTS_DIR" | sed 's/[&|\\]/\\&/g')"

    sed -i \
        -e "s|@TERMINAL@|$dir_escaped/mmt-terminal|g" \
        -e "s|@SCRIPTS_DIR@|$dir_escaped|g" \
        "$INSTALL_DIR"/multimedia-*.desktop

    chmod +x "$INSTALL_DIR"/multimedia-*.desktop
}

# Function to install
install() {
    print_header "$MSG_TITLE"

    check_dependencies

    print_step "2" "6" "$MSG_SETUP_DIR"
    # Create installation directory
    mkdir -p "$INSTALL_DIR"
    print_success "$MSG_DIR_READY"

    install_helper_scripts
    remove_legacy_files

    create_audio_menu
    create_image_menu
    create_video_menu

    finalize_desktop_files

    print_completion
}

# Function to uninstall
uninstall() {
    print_header "$MSG_UNINSTALL_TITLE"

    if [ -d "$INSTALL_DIR" ]; then
        print_info "$MSG_REMOVING_FILES"
        rm -f "$INSTALL_DIR"/multimedia-convert-*.desktop
        rm -f "$INSTALL_DIR"/multimedia-compress-*.desktop
        rm -f "$INSTALL_DIR"/multimedia-convert-video-audio.desktop
        rm -f "$INSTALL_DIR"/multimedia-delete-sound.desktop
        rm -f "$INSTALL_DIR"/multimedia-extract-audio.desktop
        rm -f "$INSTALL_DIR"/multimedia-make-frame-sequence.desktop
        rm -f "$INSTALL_DIR"/multimedia-stats.desktop
        rm -f "$INSTALL_DIR"/multimedia-video-options.desktop
        rm -f "$INSTALL_DIR"/multimedia-compress-limit.desktop
        rm -f "$INSTALL_DIR"/multimedia-convert-webp-anim.desktop
        rm -rf "$SCRIPTS_DIR"
        print_success "$MSG_FILES_REMOVED"
    else
        print_warning "$MSG_DIR_NOT_FOUND $INSTALL_DIR"
    fi

    echo
    print_success "$MSG_UNINSTALL_COMPLETE"
    print_info "$MSG_RESTART_AFTER_UNINSTALL"
}

# Function to show help
show_help() {
    cat << EOF
KDE Plasma 6 Multimedia Context Menu Installer

Usage: $0 [OPTION]

Options:
  install     Install the context menu (default)
  uninstall   Remove the context menu
  help        Show this help message

This script creates KDE Plasma 6 .desktop files for multimedia file operations
including conversion, compression, and analysis using ffmpeg/ffprobe.

Supported file types:
  Audio: WAV, MP3, FLAC, AAC, OGG, WMA, M4A, WebA, Opus
  Images: JPG, PNG, BMP, TIFF, GIF, WebP, TGA, PSD
  Video: MP4, AVI, MOV, MKV, WebM, M4V

Requirements:
  - ffmpeg and ffprobe
  - python3 with PyQt6 or PySide6 (for the dialogs)
  - KDE Plasma 6
  - Dolphin file manager
  - Any terminal emulator (detected automatically, see below)
  - curl or wget (only when the helper scripts are not next to the installer)

Helper scripts (linux/scripts/) are copied from the local repository when the
installer is run from a clone, otherwise they are downloaded from GitHub.
  MMT_BRANCH=<ref>        download from another branch/tag
  MMT_REPO_RAW_URL=<url>  download from another location (fork, mirror)

Terminal selection (first match wins):
  1. \$MMT_TERMINAL or ~/.config/multimedia-context-menu/terminal
  2. Terminal configured in KDE
  3. xdg-terminal-exec, then \$TERMINAL
  4. konsole, gnome-terminal, alacritty, kitty, xterm, ... (first installed)

EOF
}

# Main script logic
case "${1:-install}" in
    install)
        install
        ;;
    uninstall)
        uninstall
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        print_error "Invalid option: $1"
        echo "Use '$0 help' for usage information."
        exit 1
        ;;
esac
