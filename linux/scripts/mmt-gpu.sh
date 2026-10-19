#!/bin/bash
# shellcheck shell=bash
# mmt-gpu.sh - helpers for the AMD / Intel GPU encoders (VAAPI).
# Sourced by compress_mp4.sh and compress_limit.sh.

# mmt_render_node <amd|intel>
# Prints the /dev/dri render node of the first GPU of that vendor and returns 0,
# returns 1 if there is none. A computer can have several GPUs (for example an
# Intel iGPU plus an Nvidia card), so the vendor is looked up instead of
# blindly taking renderD128.
# (MMT_DRM_DIR only exists so the lookup can be tested without real hardware.)
mmt_render_node() {
    local want node
    case "$1" in
        amd)   want="0x1002" ;;
        intel) want="0x8086" ;;
        *)     return 1 ;;
    esac
    for node in "${MMT_DRM_DIR:-/sys/class/drm}"/renderD*; do
        [ -r "$node/device/vendor" ] || continue
        if [ "$(cat "$node/device/vendor")" = "$want" ]; then
            echo "/dev/dri/$(basename "$node")"
            return 0
        fi
    done
    return 1
}
