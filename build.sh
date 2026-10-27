#!/usr/bin/env bash
# build.sh - builds mmt (the Media Tools program) for Linux and Windows, x86 and ARM.
#
#   ./build.sh                  build for this computer
#   ./build.sh --all            build every target below that can be built here
#   ./build.sh linux-aarch64 windows-x86_64     build the named targets
#   ./build.sh --list           list the targets and what each one needs
#   ./build.sh --debug ...      debug instead of release build
#   ./build.sh --no-appimage    Linux: only the plain binary, no AppImage
#
# The finished files are put into ./dist: mmt-linux-x86_64.AppImage (the main Linux download),
# the plain binary mmt-linux-x86_64, mmt-windows-x86_64.exe, ...
#
# AppImage: the binary, its .desktop file and icon packed with appimagetool. ffmpeg is not
# inside: the AppImage uses the ffmpeg of the system (or ffmpeg / ffprobe next to the .AppImage).
# appimagetool is taken from APPIMAGETOOL=..., from PATH, or downloaded once into target/tools/.
#
# Targets (a full Rust target triple works as well):
#   linux-x86_64     x86_64-unknown-linux-gnu        PCs
#   linux-aarch64    aarch64-unknown-linux-gnu       64-bit ARM: Raspberry Pi 3/4/5 with 64-bit OS, ARM laptops
#   linux-armv7      armv7-unknown-linux-gnueabihf   32-bit ARM: Raspberry Pi 2/3/4 with 32-bit Raspberry Pi OS
#   windows-x86_64   x86_64-pc-windows-gnu           Windows 10/11
#   windows-aarch64  aarch64-pc-windows-gnullvm      Windows on ARM (needs zig, see below)
#
# What a cross build needs (the program has no C libraries to link, so only a linker):
#   * the Rust standard library of the target:   rustup target add <triple>   (done by this script)
#   * EITHER cargo-zigbuild + zig  (cargo install cargo-zigbuild; zig from your package manager).
#       One tool for every target, and it can aim at an OLD glibc, so the Linux binaries run on
#       old distributions, too. This script uses it automatically when it is installed.
#   * OR a GNU cross compiler per target:
#       aarch64   Arch: aarch64-linux-gnu-gcc            Debian/Ubuntu: gcc-aarch64-linux-gnu
#       armv7     Arch: arm-linux-gnueabihf-gcc          Debian/Ubuntu: gcc-arm-linux-gnueabihf
#       windows   Arch: mingw-w64-gcc                    Debian/Ubuntu: gcc-mingw-w64-x86-64
#     (a binary built this way needs a glibc at least as new as the one of the build machine)
#
# Environment: GLIBC=2.28 sets the glibc version aimed at by zigbuild (default 2.28).

set -u
cd "$(dirname "$(readlink -f "$0")")" || exit 1
OUT="dist"
GLIBC="${GLIBC:-2.28}"
PROFILE=release
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -n1)"
APPIMAGE=1

ALL_TARGETS="linux-x86_64 linux-aarch64 linux-armv7 windows-x86_64 windows-aarch64"

triple_of() {
    case "$1" in
        linux-x86_64)    echo x86_64-unknown-linux-gnu ;;
        linux-aarch64)   echo aarch64-unknown-linux-gnu ;;
        linux-armv7)     echo armv7-unknown-linux-gnueabihf ;;
        windows-x86_64)  echo x86_64-pc-windows-gnu ;;
        windows-aarch64) echo aarch64-pc-windows-gnullvm ;;
        *-*-*)           echo "$1" ;;
        *)               return 1 ;;
    esac
}

name_of() {   # triple -> friendly name used for the file in dist/
    case "$1" in
        x86_64-unknown-linux-gnu)        echo linux-x86_64 ;;
        aarch64-unknown-linux-gnu)       echo linux-aarch64 ;;
        armv7-unknown-linux-gnueabihf)   echo linux-armv7 ;;
        x86_64-pc-windows-gnu)           echo windows-x86_64 ;;
        aarch64-pc-windows-gnullvm)      echo windows-aarch64 ;;
        *)                               echo "$1" ;;
    esac
}

gnu_cc_of() {  # the GNU cross compiler to look for
    case "$1" in
        aarch64-unknown-linux-gnu)       echo aarch64-linux-gnu-gcc ;;
        armv7-unknown-linux-gnueabihf)   echo arm-linux-gnueabihf-gcc ;;
        x86_64-pc-windows-gnu)           echo x86_64-w64-mingw32-gcc ;;
        *)                               echo "" ;;
    esac
}

host_triple() { rustc -vV 2>/dev/null | sed -n 's/^host: //p'; }

have() { command -v "$1" >/dev/null 2>&1; }

usage() { sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; }

list_targets() {
    local host; host="$(host_triple)"
    echo "Targets (this computer: ${host:-unknown}):"
    for t in $ALL_TARGETS; do
        local tr cc note=""
        tr="$(triple_of "$t")"; cc="$(gnu_cc_of "$tr")"
        if [ "$tr" = "$host" ]; then note="native"
        elif have cargo-zigbuild && have zig; then note="via cargo-zigbuild"
        elif [ -n "$cc" ] && have "$cc"; then note="via $cc"
        else note="not available here (needs cargo-zigbuild + zig${cc:+, or $cc})"; fi
        printf '  %-16s %-34s %s\n' "$t" "$tr" "$note"
    done
}

appimage_arch() {  # triple -> the architecture name of appimagetool
    case "$1" in
        x86_64-*-linux-*)  echo x86_64 ;;
        aarch64-*-linux-*) echo aarch64 ;;
        armv7-*-linux-*)   echo armhf ;;
        *)                 echo "" ;;
    esac
}

find_appimagetool() {
    if [ -n "${APPIMAGETOOL:-}" ]; then echo "$APPIMAGETOOL"; return 0; fi
    if have appimagetool; then command -v appimagetool; return 0; fi
    local arch tool
    arch="$(uname -m)"
    tool="target/tools/appimagetool-$arch.AppImage"
    if [ ! -x "$tool" ]; then
        have curl || return 1
        mkdir -p target/tools
        echo "    downloading appimagetool ($arch)" >&2
        curl -fsSL -o "$tool.part" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage" \
            || { rm -f "$tool.part"; return 1; }
        chmod +x "$tool.part" && mv "$tool.part" "$tool"
    fi
    echo "$tool"
}

checksum() { if have sha256sum; then (cd "$OUT" && sha256sum "$1" > "$1.sha256"); fi; }

make_appimage() {  # binary, triple, friendly name
    local bin="$1" tr="$2" name="$3" arch tool dir icons out
    arch="$(appimage_arch "$tr")"
    [ -n "$arch" ] || return 0
    tool="$(find_appimagetool)" || { echo "    no AppImage: appimagetool not found and not downloadable (set APPIMAGETOOL=...)" >&2; return 1; }
    dir="target/appdir-$name"
    icons="$dir/usr/share/icons/hicolor/256x256/apps"
    rm -rf "$dir"
    mkdir -p "$dir/usr/bin" "$dir/usr/share/applications" "$icons"
    cp "$bin" "$dir/usr/bin/mmt"
    ln -s usr/bin/mmt "$dir/AppRun"
    cp assets/mmt.desktop "$dir/mmt.desktop"
    cp assets/mmt.desktop "$dir/usr/share/applications/mmt.desktop"
    cp assets/mmt.png "$dir/mmt.png"
    cp assets/mmt.png "$icons/mmt.png"
    ln -s mmt.png "$dir/.DirIcon"
    out="mmt-$name.AppImage"
    # extract-and-run: appimagetool is an AppImage itself and must work without FUSE (containers, CI)
    if ! ARCH="$arch" VERSION="$VERSION" APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$dir" "$OUT/$out" > "target/appimagetool-$name.log" 2>&1; then
        echo "    FAILED: AppImage (see target/appimagetool-$name.log)" >&2
        return 1
    fi
    checksum "$out"
    echo "    -> dist/$out ($(du -h "$OUT/$out" | cut -f1))"
}

build_one() {
    local t="$1" tr name ext="" host cc var
    tr="$(triple_of "$t")" || { echo "build.sh: unknown target \"$t\" (see --list)" >&2; return 1; }
    name="$(name_of "$tr")"
    case "$tr" in *windows*) ext=".exe" ;; esac
    host="$(host_triple)"
    local flags=(); [ "$PROFILE" = release ] && flags+=(--release)
    local cmd=(cargo build "${flags[@]}" --target "$tr")

    echo "==> $name ($tr)"
    if have rustup && [ "$tr" != "$host" ]; then
        rustup target add "$tr" >/dev/null 2>&1 || echo "    (rustup could not add the target; continuing)"
    fi

    cc="$(gnu_cc_of "$tr")"
    if [ "$tr" = "$host" ]; then
        :   # native build, nothing special
    elif have cargo-zigbuild && have zig; then
        case "$tr" in
            *-linux-gnu*) cmd=(cargo zigbuild "${flags[@]}" --target "$tr.$GLIBC") ;;
            *)            cmd=(cargo zigbuild "${flags[@]}" --target "$tr") ;;
        esac
    elif [ -n "$cc" ] && have "$cc"; then
        var="$(echo "$tr" | tr 'a-z-' 'A-Z_')"
        export "CARGO_TARGET_${var}_LINKER=$cc"
        export "CC_$(echo "$tr" | tr '-' '_')=$cc"
    else
        echo "    cannot build this target here: install cargo-zigbuild and zig${cc:+, or the compiler $cc}" >&2
        return 1
    fi

    echo "    ${cmd[*]}"
    "${cmd[@]}" || { echo "    FAILED: $name" >&2; return 1; }

    # always built with --target, so never take a (possibly stale) target/release/mmt
    local built="target/$tr/$PROFILE/mmt$ext"
    [ -f "$built" ] || { echo "    the binary was not found: $built" >&2; return 1; }
    mkdir -p "$OUT"
    cp "$built" "$OUT/mmt-$name$ext"
    checksum "mmt-$name$ext"
    echo "    -> dist/mmt-$name$ext ($(du -h "$OUT/mmt-$name$ext" | cut -f1))"
    if [ "$APPIMAGE" = 1 ]; then make_appimage "$built" "$tr" "$name" || return 1; fi
}

# ---------------------------------------------------------------- arguments
TARGETS=()
while [ $# -gt 0 ]; do
    case "$1" in
        -h|--help)  usage; exit 0 ;;
        --list)     list_targets; exit 0 ;;
        --all)      for t in $ALL_TARGETS; do TARGETS+=("$t"); done ;;
        --debug)    PROFILE=debug ;;
        --no-appimage) APPIMAGE=0 ;;
        --out)      shift; OUT="${1:?--out needs a directory}" ;;
        -*)         echo "build.sh: unknown option $1" >&2; usage >&2; exit 2 ;;
        *)          TARGETS+=("$1") ;;
    esac
    shift
done

have cargo || { echo "build.sh: cargo is not installed (https://rustup.rs)" >&2; exit 1; }
[ ${#TARGETS[@]} -gt 0 ] || TARGETS=("$(host_triple)")

echo "mmt $VERSION, $PROFILE build"
ok=0; failed=0
for t in "${TARGETS[@]}"; do
    if build_one "$t"; then ok=$((ok + 1)); else failed=$((failed + 1)); fi
done
echo
echo "built: $ok, failed or skipped: $failed   (files are in dist/)"
[ "$failed" -eq 0 ] || { [ "${#TARGETS[@]}" -gt 1 ] && [ "$ok" -gt 0 ] && exit 0; exit 1; }
