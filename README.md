# Media Tools (`cmmt`)

Adds ffmpeg actions to the right-click menu of your file manager: compress, convert, extract audio and more.
Works with Windows Explorer, KDE (Dolphin), GNOME (Files), XFCE (Thunar) and Raspberry Pi OS (PCManFM), on x86 and ARM.

The only requirement is `ffmpeg` with `ffprobe`. Linux gets one AppImage, Windows gets one `.exe`.
Every ffmpeg command lives in a text file, so you can edit any of them.

## Install

**Linux**

1. Install `ffmpeg` with your package manager.
2. Download the AppImage for your machine:
   `cmmt-linux-x86_64.AppImage` (PC), `cmmt-linux-aarch64.AppImage` (64-bit ARM, Raspberry Pi 3/4/5),
   `cmmt-linux-armv7.AppImage` (32-bit Raspberry Pi OS).
3. Make it executable, start it and press **Install** in the window that opens. Or in a terminal:

   ```
   chmod +x cmmt-linux-x86_64.AppImage
   ./cmmt-linux-x86_64.AppImage install     # detects the desktop; or --kde --gnome --xfce --pcmanfm
   ./cmmt-linux-x86_64.AppImage uninstall
   ```

The menu items point to the AppImage where it is. If you move it, press **Install** again.
On XFCE and PCManFM, restart the file manager once (`thunar -q`, `pcmanfm -q`).
On GNOME, the items are under **Scripts → Media Tools**.

**Windows**

Put `ffmpeg.exe` and `ffprobe.exe` next to `cmmt.exe` or in `PATH`, start `cmmt.exe` and press **Install**.
On Windows 11 the items are under *Show more options*.

`install --path` also makes `cmmt` available in a terminal.

## Menu items

| Item | What it does |
|---|---|
| Compress Video | quality level or a size limit in MB (e.g. Discord), codec on CPU or GPU (Nvidia, AMD, Intel, Raspberry Pi), resolution |
| Convert Video | MP4, AVI, MOV, MKV, M4V, WebM |
| Convert Audio | WAV, MP3, FLAC, AAC, OGG, WMA, M4A, Opus; for videos, only the audio tracks are converted |
| Extract Audio | MP3, AAC, Opus, WAV, M4A, MKA |
| Delete sound | the video without audio |
| Make frame sequence | PNG / JPG / WebP frames in a new folder |
| Convert to WebP anim | size, fps, quality, compression |
| Convert Image | JPG, PNG, BMP, TIFF, GIF, WebP, TGA |
| Stats | duration, size, bitrate and streams of the files, plus the full `ffprobe` output |

Existing files are never overwritten: the result gets a name like `clip_compressed (1).mp4`.

## Settings

Start `cmmt` without arguments or open **Media Tools** from the application menu.
Here you set the theme, accent colour and interface scale, turn off menu items you don't need, add your own items
with any ffmpeg command, and set the paths to ffmpeg / ffprobe.

Once a day the Settings window checks GitHub for a newer release (with `curl`).
If there is one, a dot and a link appear above the version number. You can turn this off under *General → Updates*.

Settings are stored in `~/.config/cmmt/` (Windows: `%APPDATA%\cmmt\`).

## Editing the commands

```
cmmt config --init    # writes presets.toml with all commands to the settings folder
```

Each action is a table with a full ffmpeg command line:

```toml
[compress.h264_cpu]
label = "H.264"
command = 'ffmpeg -n -i {input} {vf} -pix_fmt yuv420p -c:v libx264 -crf {q} {output}'
output = "{base}_compressed.mp4"
q = [22, 30, 35]    # low / medium / high
```

Your file only needs the entries you changed, and everything else comes from the built-in defaults.
If the file contains an error, the program ignores it and shows what is wrong.
The comments in the file list the placeholders (`{input}`, `{output}`, `{base}`, `{vf}`, `{q}`, …) and options.
To add a format or preset, add a new table.

## Command line

```
cmmt compress --cli --limit 20 --codec h265_cpu video.mp4    # no window, progress in the terminal
cmmt convert-audio --yes --format mp3 song.wav               # no options dialog
cmmt help
```

## Build

```
./build.sh            # for this computer
./build.sh --all      # every target that can be built here
./build.sh --list     # what each target needs
```

Requires Rust. Cross builds use `cargo-zigbuild` + `zig`. The results go to `dist/`.

## Status

Tested on Linux x86_64 (KDE, GNOME, XFCE). Not tested yet: Windows, Raspberry Pi / PCManFM, ARM builds,
AMD / Intel GPU encoding.
