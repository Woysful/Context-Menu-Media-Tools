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
    print_step "1" "5" "$MSG_CHECKING_DEPS"

    local deps=("ffmpeg" "ffprobe")
    local missing_deps=()

    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            missing_deps+=("$dep")
        fi
    done

    if [ ${#missing_deps[@]} -ne 0 ]; then
        print_error "$MSG_MISSING_DEPS ${missing_deps[*]}"
        print_info "$MSG_INSTALL_DEPS"
        print_info "  Arch Linux: sudo pacman -S ffmpeg"
        print_info "  Ubuntu/Debian: sudo apt install ffmpeg"
        exit 1
    fi

    print_success "$MSG_DEPS_FOUND"
}


# Function to create audio context menu
create_audio_menu() {
    print_step "2" "5" "$MSG_CREATING_AUDIO"

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
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a pcm_s16le "${file%.*}.wav"; done' bash %F
Icon=audio-wav

[Desktop Action convertMP3]
Name=MP3
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -ab 192k -map_metadata 0 -id3v2_version 3 "${file%.*}.mp3"; done' bash %F
Icon=audio-mpeg

[Desktop Action convertFLAC]
Name=FLAC
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a flac "${file%.*}.flac"; done' bash %F
Icon=audio-flac

[Desktop Action convertAAC]
Name=AAC
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a aac -b:a 192k "${file%.*}.aac"; done' bash %F
Icon=audio-aac

[Desktop Action convertOGG]
Name=OGG
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a libvorbis -q:a 0 "${file%.*}.ogg"; done' bash %F
Icon=audio-ogg

[Desktop Action convertWMA]
Name=WMA
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a wmav2 -b:a 192k "${file%.*}.wma"; done' bash %F
Icon=audio-x-ms-wma

[Desktop Action convertM4A]
Name=M4A
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:a aac -b:a 192k "${file%.*}.m4a"; done' bash %F
Icon=audio-mp4

[Desktop Action convertOpus]
Name=Opus
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -q:a 0 -c:a libopus "${file%.*}.opus"; done' bash %F
Icon=audio-opus

[Desktop Action statsAudio]
Name=Stats
Name[ru]=Детали
Name[uk]=Деталі
Exec=konsole --hold -e bash -c 'for file in "$@" ; do ffprobe -hide_banner -i "$file"; echo "Press Enter to continue..."; read; done' bash %F
Icon=documentinfo
EOF

    print_success "$MSG_AUDIO_CREATED"
}

# Function to create image context menu
create_image_menu() {
    print_step "3" "5" "$MSG_CREATING_IMAGE"

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
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -q:v 5 "${file%.*}.jpg"; done' bash %F
Icon=image-jpeg

[Desktop Action convertPNG]
Name=PNG
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" "${file%.*}.png"; done' bash %F
Icon=image-png

[Desktop Action convertBMP]
Name=BMP
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" "${file%.*}.bmp"; done' bash %F
Icon=image-bmp

[Desktop Action convertTIFF]
Name=TIFF
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" "${file%.*}.tiff"; done' bash %F
Icon=image-tiff

[Desktop Action convertGIF]
Name=GIF
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -f gif "${file%.*}.gif"; done' bash %F
Icon=image-gif

[Desktop Action convertWebP]
Name=WebP
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libwebp -q:v 95 "${file%.*}.webp"; done' bash %F
Icon=image-webp

[Desktop Action convertTGA]
Name=TGA
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" "${file%.*}.tga"; done' bash %F
Icon=image-x-tga

[Desktop Action statsImage]
Name=Stats
Name[ru]=Детали
Name[uk]=Деталі
Exec=konsole --hold -e bash -c 'for file in "$@" ; do ffprobe -hide_banner -i "$file"; echo "Press Enter to continue..."; read; done' bash %F
Icon=documentinfo
EOF

    print_success "$MSG_IMAGE_CREATED"
}

# Function to create video context menu
create_video_menu() {
    print_step "4" "5" "$MSG_CREATING_VIDEO"

    # Video Convert Menu
    cat > "$INSTALL_DIR/multimedia-convert-video.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=convertMP4;convertAVI;convertMOV;convertMKV;convertM4V;convertWebM
X-KDE-Submenu=Convert Video
X-KDE-Submenu[ru]=Конвертировать
X-KDE-Submenu[uk]=Конвертувати
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action convertMP4]
Name=MP4
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v copy -c:a aac "${file%.*}.mp4"; done' bash %F
Icon=video-mp4

[Desktop Action convertAVI]
Name=AVI
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v mpeg4 -q:v 2 "${file%.*}.avi"; done' bash %F
Icon=video-x-msvideo

[Desktop Action convertMOV]
Name=MOV
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v copy "${file%.*}.mov"; done' bash %F
Icon=video-quicktime

[Desktop Action convertMKV]
Name=MKV
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v copy -c:a copy "${file%.*}.mkv"; done' bash %F
Icon=video-x-matroska

[Desktop Action convertM4V]
Name=M4V
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v copy -c:a aac "${file%.*}.m4v"; done' bash %F
Icon=video-mp4

[Desktop Action convertWebM]
Name=WebM
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libvpx-vp9 -c:a libopus "${file%.*}.webm"; done' bash %F
Icon=video-webm
EOF

    # Delete Sound Menu
    cat > "$INSTALL_DIR/multimedia-delete-sound.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=deleteSound
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action deleteSound]
Name=Delete sound
Name[ru]=Удалить звук
Name[uk]=Видалити звук
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -an -c copy "${file%.*} (no audio).${file##*.}"; done' bash %F
Icon=media-track-remove-amarok
EOF

    # Extract Audio Menu
    cat > "$INSTALL_DIR/multimedia-extract-audio.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=extractMP3;extractAAC;extractOpus;extractWAV;extractM4A;extractMKA
X-KDE-Submenu=Extract Audio
X-KDE-Submenu[ru]=Извлечь аудио
X-KDE-Submenu[uk]=Витягнути аудіо
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action extractMP3]
Name=Mp3
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vn -q:a 0 "${file%.*}.mp3"; done' bash %F
Icon=audio-mpeg

[Desktop Action extractAAC]
Name=AAC
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vn -c copy "${file%.*}.aac"; done' bash %F
Icon=audio-aac

[Desktop Action extractOpus]
Name=Opus
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vn -c:a libopus -q:a 0 -vbr on "${file%.*}.opus"; done' bash %F
Icon=audio-opus

[Desktop Action extractWAV]
Name=WAV
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vn -q:a 0 "${file%.*}.wav"; done' bash %F
Icon=audio-wav

[Desktop Action extractM4A]
Name=M4A
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vn -q:a 0 -map a -map_metadata 0 "${file%.*}.m4a"; done' bash %F
Icon=audio-mp4

[Desktop Action extractMKA]
Name=MKA
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vn -c:a copy -map a -map_metadata 0 "${file%.*}.mka"; done' bash %F
Icon=audio-x-matroska
EOF

    # Make Frame Sequence Menu
    cat > "$INSTALL_DIR/multimedia-make-frame-sequence.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=makePNG;makeJPG;makeWebP
X-KDE-Submenu=Make frame sequence
X-KDE-Submenu[ru]=Разбить на кадры
X-KDE-Submenu[uk]=Розбити на кадри
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action makePNG]
Name=PNG
Exec=konsole -e bash -c 'for file in "$@" ; do mkdir -p "${file%.*}"; ffmpeg -i "$file" "${file%.*}/frame_%06d.png"; done' bash %F
Icon=image-png

[Desktop Action makeJPG]
Name=JPG
Exec=konsole -e bash -c 'for file in "$@" ; do mkdir -p "${file%.*}"; ffmpeg -i "$file" -q:v 1 "${file%.*}/frame_%06d.jpg"; done' bash %F
Icon=image-jpeg

[Desktop Action makeWebP]
Name=WebP
Exec=konsole -e bash -c 'for file in "$@" ; do mkdir -p "${file%.*}"; ffmpeg -i "$file" -c:v libwebp -q:v 95 -pix_fmt yuva420p "${file%.*}/frame_%06d.webp"; done' bash %F
Icon=image-webp
EOF

    # Compress to MP4 Menu (Good Quality)
    cat > "$INSTALL_DIR/multimedia-compress-mp4.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=compressH264;compressH264GpuNvidia;compressH265;compressH265GpuNvidia;compressAV1CpuFast;compressAV1CpuSlow;compressAV1GpuNvidia
X-KDE-Submenu=Compress to MP4
X-KDE-Submenu[ru]=Сжать в MP4
X-KDE-Submenu[uk]=Стиснути в MP4
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action compressH264]
Name=H.264 [CPU]
Name[ru]=H.264 [CPU]
Name[uk]=H.264 [CPU]
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vf format=yuv420p -c:v libx264 -profile:v main -crf 22 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH264GpuNvidia]
Name=H.264 [GPU] Nvidia
Name[ru]=H.264 [GPU] Nvidia
Name[uk]=H.264 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -vf format=yuv420p -c:v h264_nvenc -profile:v main -cq 29 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH265]
Name=H.265 [CPU]
Name[ru]=H.265 [CPU]
Name[uk]=H.265 [CPU]
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vf format=yuv420p -c:v libx265 -profile:v main -crf 22 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH265GpuNvidia]
Name=H.265 [GPU] Nvidia
Name[ru]=H.265 [GPU] Nvidia
Name[uk]=H.265 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -vf format=yuv420p -c:v hevc_nvenc -profile:v main -cq 28 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1CpuFast]
Name=AV1 [CPU] Fast
Name[ru]=AV1 [CPU] Быстрый
Name[uk]=AV1 [CPU] Швидкий
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libsvtav1 -preset 7 -rc vbr -crf 32 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1CpuSlow]
Name=AV1 [CPU] Slow
Name[ru]=AV1 [CPU] Медленный
Name[uk]=AV1 [CPU] Повільний
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libsvtav1 -preset 5 -rc vbr -crf 32 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1GpuNvidia]
Name=AV1 [GPU] Nvidia
Name[ru]=AV1 [GPU] Nvidia
Name[uk]=AV1 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -c:v av1_nvenc -cq 37 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress
EOF

    # Compress to MP4+ Menu (Medium Quality)
    cat > "$INSTALL_DIR/multimedia-compress-mp4-plus.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=compressH264;compressH264GpuNvidia;compressH265;compressH265GpuNvidia;compressAV1CpuFast;compressAV1CpuSlow;compressAV1GpuNvidia
X-KDE-Submenu=Compress to MP4+
X-KDE-Submenu[ru]=Сжать в MP4+
X-KDE-Submenu[uk]=Стиснути в MP4+
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action compressH264]
Name=H.264 [CPU]
Name[ru]=H.264 [CPU]
Name[uk]=H.264 [CPU]
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vf format=yuv420p -c:v libx264 -profile:v main -crf 30 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH264GpuNvidia]
Name=H.264 [GPU] Nvidia
Name[ru]=H.264 [GPU] Nvidia
Name[uk]=H.264 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -vf format=yuv420p -c:v h264_nvenc -profile:v main -cq 37 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH265]
Name=H.265 [CPU]
Name[ru]=H.265 [CPU]
Name[uk]=H.265 [CPU]
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vf format=yuv420p -c:v libx265 -profile:v main -crf 30 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH265GpuNvidia]
Name=H.265 [GPU] Nvidia
Name[ru]=H.265 [GPU] Nvidia
Name[uk]=H.265 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -vf format=yuv420p -c:v hevc_nvenc -profile:v main -cq 37 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1CpuFast]
Name=AV1 [CPU] Fast
Name[ru]=AV1 [CPU] Быстрый
Name[uk]=AV1 [CPU] Швидкий
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libsvtav1 -preset 7 -rc vbr -crf 44 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1CpuSlow]
Name=AV1 [CPU] Slow
Name[ru]=AV1 [CPU] Медленный
Name[uk]=AV1 [CPU] Повільний
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libsvtav1 -preset 5 -rc vbr -crf 44 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1GpuNvidia]
Name=AV1 [GPU] Nvidia
Name[ru]=AV1 [GPU] Nvidia
Name[uk]=AV1 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -c:v av1_nvenc -cq 47 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress
EOF

    # Compress to MP4++ Menu (Bad Quality/Small Size)
    cat > "$INSTALL_DIR/multimedia-compress-mp4-plus-plus.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=compressH264;compressH264GpuNvidia;compressH265;compressH265GpuNvidia;compressAV1CpuFast;compressAV1CpuSlow;compressAV1GpuNvidia
X-KDE-Submenu=Compress to MP4++
X-KDE-Submenu[ru]=Сжать в MP4++
X-KDE-Submenu[uk]=Стиснути в MP4++
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action compressH264]
Name=H.264 [CPU]
Name[ru]=H.264 [CPU]
Name[uk]=H.264 [CPU]
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vf format=yuv420p -c:v libx264 -profile:v main -crf 35 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH264GpuNvidia]
Name=H.264 [GPU] Nvidia
Name[ru]=H.264 [GPU] Nvidia
Name[uk]=H.264 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -vf format=yuv420p -c:v h264_nvenc -profile:v main -cq 41 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH265]
Name=H.265 [CPU]
Name[ru]=H.265 [CPU]
Name[uk]=H.265 [CPU]
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -vf format=yuv420p -c:v libx265 -profile:v main -crf 35 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressH265GpuNvidia]
Name=H.265 [GPU] Nvidia
Name[ru]=H.265 [GPU] Nvidia
Name[uk]=H.265 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -vf format=yuv420p -c:v hevc_nvenc -profile:v main -cq 43 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1CpuFast]
Name=AV1 [CPU] Fast
Name[ru]=AV1 [CPU] Быстрый
Name[uk]=AV1 [CPU] Швидкий
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libsvtav1 -preset 5 -rc vbr -crf 55 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1CpuSlow]
Name=AV1 [CPU] Slow
Name[ru]=AV1 [CPU] Медленный
Name[uk]=AV1 [CPU] Повільний
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -c:v libsvtav1 -preset 4 -rc vbr -crf 55 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress

[Desktop Action compressAV1GpuNvidia]
Name=AV1 [GPU] Nvidia
Name[ru]=AV1 [GPU] Nvidia
Name[uk]=AV1 [GPU] Nvidia
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -hwaccel cuda -hwaccel_output_format cuda -i "$file" -c:v av1_nvenc -cq 56 -preset p7 "${file%.*}_compressed.mp4"; done' bash %F
Icon=video-compress
EOF

    # Convert Audio Menu (Video)
    cat > "$INSTALL_DIR/multimedia-convert-video-audio.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=convertAudioWAV;convertAudioMP3;convertAudioFLAC;convertAudioAAC;convertAudioOGG;convertAudioWMA;convertAudioM4A;convertAudioOpus
X-KDE-Submenu=Convert Audio
X-KDE-Submenu[ru]=Конвертировать аудио
X-KDE-Submenu[uk]=Конвертувати аудіо
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action convertAudioWAV]
Name=WAV
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -map 0:v -c:v copy -map 0:a -c:a pcm_s16le "${file%.*}_audio_wav.${file##*.}"; done' bash %F
Icon=audio-wav

[Desktop Action convertAudioMP3]
Name=MP3
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -map 0:v -c:v copy -map 0:a -c:a mp3 -b:a 192k "${file%.*}_audio_mp3.${file##*.}"; done' bash %F
Icon=audio-mpeg

[Desktop Action convertAudioFLAC]
Name=FLAC
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -map 0:v -c:v copy -map 0:a -c:a flac "${file%.*}_audio_flac.${file##*.}"; done' bash %F
Icon=audio-flac

[Desktop Action convertAudioAAC]
Name=AAC
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -map 0:v -c:v copy -map 0:a -c:a aac -b:a 192k "${file%.*}_audio_aac.${file##*.}"; done' bash %F
Icon=audio-aac

[Desktop Action convertAudioOGG]
Name=OGG
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -map 0:v -c:v copy -map 0:a -c:a libvorbis -q:a 0 "${file%.*}_audio_ogg.${file##*.}"; done' bash %F
Icon=audio-ogg

[Desktop Action convertAudioWMA]
Name=WMA
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -map 0:v -c:v copy -map 0:a -c:a wmav2 -b:a 192k "${file%.*}_audio_wma.${file##*.}"; done' bash %F
Icon=audio-x-ms-wma

[Desktop Action convertAudioM4A]
Name=M4A
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -map 0:v -c:v copy -map 0:a -c:a aac -b:a 192k "${file%.*}_audio_m4a.${file##*.}"; done' bash %F
Icon=audio-mp4

[Desktop Action convertAudioOpus]
Name=Opus
Exec=konsole -e bash -c 'for file in "$@" ; do ffmpeg -i "$file" -map 0:v -c:v copy -map 0:a -c:a libopus -q:a 0 "${file%.*}_audio_opus.${file##*.}"; done' bash %F
Icon=audio-opus
EOF

    # Stats Menu
    cat > "$INSTALL_DIR/multimedia-stats.desktop" << 'EOF'
[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=audio/wav;audio/mpeg;audio/flac;audio/aac;audio/ogg;audio/x-ms-wma;audio/mp4;audio/webm;audio/opus;image/jpeg;image/png;image/bmp;image/tiff;image/gif;image/webp;image/x-tga;image/vnd.adobe.photoshop;video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm
Actions=stats
X-KDE-Priority=TopLevel
X-KDE-AuthorizeAction=shell_access

[Desktop Action stats]
Name=Stats
Name[ru]=Детали
Name[uk]=Деталі
Exec=konsole --hold -e bash -c 'for file in "$@" ; do ffprobe -hide_banner -i "$file"; echo "Press Enter to continue..."; read; done' bash %F
Icon=documentinfo
EOF

    # Set executable permissions for all desktop files
    chmod +x "$INSTALL_DIR"/*.desktop

    print_success "$MSG_VIDEO_CREATED"
}

# Function to install
install() {
    print_header "$MSG_TITLE"

    check_dependencies

    print_step "5" "5" "$MSG_SETUP_DIR"
    # Create installation directory
    mkdir -p "$INSTALL_DIR"
    print_success "$MSG_DIR_READY"

    create_audio_menu
    create_image_menu
    create_video_menu

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
  - KDE Plasma 6
  - Dolphin file manager

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
