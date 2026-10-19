# Context Menu Media Tools
FFmpeg based tools that baked into context menu to compress, convert media files, delete, extract or convert sound, and look for detailed stats with ffprobe.

![](https://github.com/user-attachments/assets/3b708629-df98-4388-a80e-d3994485d8fc)

It was created for my personal use, but you may find it useful too.

## Features

### Audio Files (WAV, MP3, FLAC, AAC, OGG, WMA, M4A, WebM, Opus)
- **Convert Audio** - Convert to various formats (WAV, MP3, FLAC, AAC, OGG, WMA, M4A, Opus)

### Images (JPG, PNG, BMP, TIFF, GIF, WebP, TGA, PSD)
- **Convert Image** - Convert to various formats (JPG, PNG, BMP, TIFF, GIF, WebP, TGA)

### Video Files (MP4, AVI, MOV, MKV, WebM, M4V)
Items are listed in this order on Linux (Windows has fewer items, see the note below):

- **Compress to MP4** - H.264, HEVC, AV1 | + Nvidia GPU encoding (on Linux also AMD and Intel)
  - Linux: a single item, one dialog asks for the *compression level* (Low / Medium / High) and the *video codec*
    (drop-down list with the groups CPU, GPU · Nvidia, GPU · AMD and GPU · Intel)
  - Windows: separate **MP4 / MP4+ / MP4++** items (High / Balanced / Low bitrate)
- **Compress to Limit** *(Linux)* - Encode to a target file size. One dialog asks for the *size limit* (MB), the
  *video codec* (Discord preset / CPU / GPU · Nvidia / AMD / Intel) and an optional *two-pass* switch. The Discord preset fixes the limit to 20 MB.
  - The size is computed from the exact duration and frame rate, the audio track is accounted for only if the video has one,
    and the container overhead is estimated instead of using a flat safety margin. "MB" means MiB (1 048 576 bytes).
  - By default the video is encoded in a single pass. The bitrate is calculated in advance, so the result depends on the
    content: on long videos it usually lands within 1-3% of the limit, on short videos (a few seconds) and on videos with
    sudden changes of complexity it can miss by more. For that the encoder aims slightly below the limit
    (more so for short videos). AV1 (CPU) is the least predictable.
  - **Two-pass** (H.264 / HEVC on CPU only) analyses the video first and then encodes it once, so the size is predictable:
    the result lands 2-5% below the limit and did not exceed it in tests. It takes longer; nothing is encoded twice,
    so there is no quality loss. GPU encoders and AV1 ignore the switch.
  - **Resolution** (optional, default: original). Drop-down list with *Auto*, popular sizes (1440p ... 360p, counted by the
    shorter side, so vertical videos work too) and scales (75%, 50%, 33%, 25%). You can also type your own value:
    `50%`, `x0.5`, `0.5` (a scale factor) or `540p` / `540` (shorter side in pixels). Nothing is ever scaled up.
    A lower resolution means more bits per pixel at the same file size, i.e. a cleaner picture.
    *Auto* picks, for every file, the largest size that still gets enough bits per pixel at the chosen limit
    (0.07 for H.264, 0.05 for HEVC and AV1; frame rates above 30 count at half weight). These are rules of thumb:
    static content such as screen recordings needs less, so check the first result and choose a size manually if needed.
  - The dialog shows the resolution, frame rate, bitrates and duration of the selected files and previews the resulting size.
  - **GPU encoders (Nvidia, AMD, Intel)** only approximate the requested bitrate, by an amount that depends on the card, the
    driver and the footage. They are therefore encoded in chunks (about 6 s each, the last one shorter): after every chunk the real
    size is measured and the bitrate of the remaining chunks is corrected. Every frame is still encoded exactly once and the chunks
    are joined without re-encoding. With a software encoder whose bitrate was deliberately distorted by +-10% the result stayed
    between -4% and +0.2% of the limit (it has not been measured on real GPUs yet). An encoder that produces more than requested is
    corrected fully, one that produces less only a little, because going over the limit is worse than falling short.
  - At the end the terminal prints the exact file size and warns if it is over the limit. The window closes by itself after a
    successful run and stays open (press Enter) only if ffmpeg reported an error.
- **Convert Video** - Convert to other formats (MP4, AVI, MOV, MKV, WebM, M4V)
- **Convert Audio** - Convert all audio tracks (WAV, MP3, FLAC, AAC, OGG, WMA, M4A, Opus)
- **Extract Audio** - Extract audio track (MP3, AAC, Opus, WAV, M4A, MKA)
- **Delete Sound** - Remove audio track from video
- **Make Frame Sequence** - Create frame sequence (PNG, JPG, WebP)
- **Convert to WebP anim** *(Linux)* - Convert video to a looping animated WebP without sound. One dialog asks for:
  - *size limit* (default 720) - the shorter side is scaled down to this value, aspect ratio is kept, no upscaling
  - *FPS* (default 20), *quality* (default 80), *compression level* (default 5)

On Linux, Convert Video, Convert Audio, Extract Audio and Make Frame Sequence are single items too:
the output format is picked from a drop-down list in a small dialog.

**Stats** - View file information using ffprobe

> Windows version has much less options because of it's context menu item limits.
> Only h265 [CPU], AV1[CPU] and AV1[GPU] presented

## 🐧 Installation Linux

### Requirements
- KDE Plasma 6
- Dolphin file manager
- ffmpeg
- ffprobe
- python3 with PyQt6 or PySide6 (draws the dialogs, uses your KDE theme)
- Any terminal emulator (konsole, gnome-terminal, alacritty, kitty, xterm, ... - detected automatically)
- curl or wget (only to download the helper scripts, see below)
- Optional, for GPU encoding: a Nvidia card with NVENC, or an AMD / Intel GPU with working VAAPI
  (`vainfo` lists encode profiles, e.g. `VAProfileH264Main : VAEntrypointEncSlice`). Your user must be allowed to use the
  GPU (group `render` or `video`), and ffmpeg must be built with VAAPI (the usual distribution packages are).

### Install Dependencies

**Arch Linux:**
```bash
sudo pacman -S ffmpeg python-pyqt6
```

**Ubuntu/Debian:**
```bash
sudo apt install ffmpeg python3-pyqt6
```

### Install Context Menu
1. Download [`ContextMenu_MediaTools_KDE6-Dolphin.sh`](https://github.com/Woysful/Context-Menu-Media-Tools/blob/main/ContextMenu_MediaTools_KDE6-Dolphin.sh)
2. Run it in terminal:
```bash
chmod +x ./ContextMenu_MediaTools_KDE6-Dolphin.sh

./ContextMenu_MediaTools_KDE6-Dolphin.sh
```
To make it work restart Dolphin or log out and back in

The installer only creates the menu entries. The scripts they use (`linux/scripts/` in this repository) are
installed to `~/.local/share/multimedia-context-menu-scripts/`:
- if you cloned the repository, they are copied from your local clone;
- if you downloaded only the installer, they are fetched from GitHub
  (`MMT_BRANCH=<branch-or-tag>` selects another branch, `MMT_REPO_RAW_URL=<url>` another source, e.g. a fork).

### How to uninstall

```bash
./ContextMenu_MediaTools_KDE6-Dolphin.sh uninstall
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

Basicaly each menu item runs ffmpeg terminal command with pre-made settings. On Linux the items that need
choices first show a single dialog (`mmt-dialog`), then run ffmpeg in a terminal. The terminal automatically closes after completion, except for the Stats command which keeps it open for you to review the information.

# Troubleshooting: Linux

### Menu doesn't appear
1. Restart Dolphin: `killall dolphin && dolphin`
2. Or log out and back in
3. Check that files are created in `~/.local/share/kio/servicemenus/`
4. Update KDE cache: `kbuildsycoca6 --noincremental`

### Commands don't work
1. Ensure ffmpeg and a Qt binding for Python are installed: `ffmpeg -version`, `python3 -c "import PyQt6.QtWidgets"` (or `import PySide6.QtWidgets`)
2. Check that a terminal emulator can be found (see below)

### Wrong terminal opens / no terminal found
The terminal is picked in this order:
1. `$MMT_TERMINAL` or the first line of `~/.config/multimedia-context-menu/terminal`
2. The terminal configured in KDE (System Settings → Default Applications)
3. `xdg-terminal-exec`, then `$TERMINAL`
4. The first installed one of: konsole, gnome-terminal, ptyxis, kgx, xfce4-terminal, mate-terminal, lxterminal, qterminal, alacritty, kitty, foot, wezterm, tilix, terminator, xterm, urxvt, st

To force a specific terminal:
```bash
mkdir -p ~/.config/multimedia-context-menu
echo "alacritty" > ~/.config/multimedia-context-menu/terminal
```

### File permission errors
Reinstall the menu - the script automatically sets correct permissions.

Or set `chmod +x ~/.local/share/kio/servicemenus/*.desktop` by yourself

## Localization

Linux version supports English, Russian, and Ukrainian languages. KDE automatically selects the appropriate language based on system settings.

## Menu order (Linux)

Dolphin lists submenus before single items and sorts single items by the identifier of the action
(the name in `Actions=` of the `.desktop` file). That is why every video item is a single item with a numeric
prefix (`01_compressMP4`, `02_compressLimit`, ... `09_stats`): it keeps the order fixed. To reorder the menu, change
the prefixes in the installer and reinstall.

## GPU encoding (Linux)

The codec drop-down is grouped by hardware, so the names inside the groups are just the codecs.
- **Nvidia** uses NVENC (`h264_nvenc`, `hevc_nvenc`, `av1_nvenc`).
- **AMD** and **Intel** use VAAPI (`h264_vaapi`, `hevc_vaapi`, `av1_vaapi`). The video is decoded on the CPU and uploaded to the
  GPU, so any source format works. The GPU is picked by vendor, so a computer with several GPUs (for example an Intel iGPU and a
  Nvidia card) uses the right one. AV1 encoding needs a recent GPU (AMD RDNA 3 / Radeon RX 7000 or newer, Intel Arc or newer).
- The quality numbers for AMD and Intel in *Compress to MP4* are starting values; they are at the top of the `case` block in
  `compress_mp4.sh` (`pick '29 37 41'` = low / medium / high) if you want a different balance.
