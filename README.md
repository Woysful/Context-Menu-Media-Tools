# Context Menu Media Tools
FFmpeg based tools that baked into context menu to compress, convert media files, delete, extract or convert sound, and look for detailed stats with ffprobe.

It was created for my personal use, but you may find it useful too.

## Features

### Audio Files (WAV, MP3, FLAC, AAC, OGG, WMA, M4A, WebM, Opus)
- **Convert Audio** - Convert to various formats (WAV, MP3, FLAC, AAC, OGG, WMA, M4A, Opus)

### Images (JPG, PNG, BMP, TIFF, GIF, WebP, TGA, PSD)
- **Convert Image** - Convert to various formats (JPG, PNG, BMP, TIFF, GIF, WebP, TGA)

### Video Files (MP4, AVI, MOV, MKV, WebM, M4V)
- **Compress to MP4 / MP4+ / MP4++**  - (High / Balanced / Low bitrate)
  - H.264, H.265, AV1 | + Nvidia GPU encoging
- **Convert Video** - Convert to other formats (MP4, AVI, MOV, MKV, WebM, M4V)
- **Convert Audio** - Convert all audio tracks (WAV, MP3, FLAC, AAC, OGG, WMA, M4A, Opus)
- **Delete Sound** - Remove audio track from video
- **Extract Audio** - Extract audio track (MP3, AAC, Opus, WAV, M4A, MKA)
- **Make Frame Sequence** - Create frame sequence (PNG, JPG, WebP)

**Stats** - View file information using ffprobe

> Windows version has much less options because of it's context menu item limits.
> Only h265 [CPU], AV1[CPU] and AV1[GPU] presented

## 🐧 Installation Linux

### Requirements
- KDE Plasma 6
- Dolphin file manager
- ffmpeg
- ffprobe

### Install Dependencies

**Arch Linux:**
```bash
sudo pacman -S ffmpeg
```

**Ubuntu/Debian:**
```bash
sudo apt install ffmpeg
```

### Install Context Menu
1. Download [`ContextMenu_MediaTools_KDE6-Dolphin.sh`](https://github.com/Woysful/Context-Menu-Media-Tools/blob/main/ContextMenu_MediaTools_KDE6-Dolphin.sh)
2. Run it in terminal:
```bash
chmod +x ./ContextMenu_MediaTools_KDE6.sh

./ContextMenu_MediaTools_KDE6.sh
```
To make it work restart Dolphin or log out and back in

### How to uninstall

```bash
./ContextMenu_MediaTools_KDE6.sh uninstall
```

## 🪟 Installation Windows

### Requirements
- ffmpeg
- ffprobe

### Install Dependencies

1. Download [`ffmpeg build`](https://ffmpeg.org/download.html)
2. Put `ffmpeg.exe` and `ffprobe.exe` in **"windows"** folder on system drive. Or setup ffmpeg PATH in environment variables by yourself.

### Install Context Menu
 1. Download [`ContextMenu_MediaTools_Windows.reg`](https://github.com/Woysful/Context-Menu-Media-Tools/blob/main/ContextMenu_MediaTools_Windows.reg)
 2. Make a backup of your registry and then run the file

To make it work you need to reboot `file explorer` or your PC

### How to uninstall
Download and run [`ContextMenu_MediaTools_Windows_Uninstall.reg`](https://github.com/Woysful/Context-Menu-Media-Tools/blob/main/ContextMenu_MediaTools_Windows_Uninstall.reg)

## How It Works

Basicaly each menu item runs ffmpeg terminal command with pre-made settings. The terminal automatically closes after completion, except for the Stats command which keeps it open for you to review the information.

# Troubleshooting: Linux

### Menu doesn't appear
1. Restart Dolphin: `killall dolphin && dolphin`
2. Or log out and back in
3. Check that files are created in `~/.local/share/kio/servicemenus/`
4. Update KDE cache: `kbuildsycoca6 --noincremental`

### Commands don't work
1. Ensure ffmpeg is installed: `ffmpeg -version`
2. Check that konsole is installed: `konsole --version`

### File permission errors
Reinstall the menu - the script automatically sets correct permissions.

Or set `chmod +x ~/.local/share/kio/servicemenus/*.desktop` by yourself

## Localization

Linux version supports English, Russian, and Ukrainian languages. KDE automatically selects the appropriate language based on system settings.
