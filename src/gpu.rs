//! Which GPUs does the computer have? Used to hide the codecs of absent GPUs and to find
//! the VAAPI render device (Linux). On other systems nothing is known and every codec is listed.

use std::collections::HashSet;

pub const VENDOR_AMD: &str = "0x1002";
pub const VENDOR_INTEL: &str = "0x8086";
pub const VENDOR_NVIDIA: &str = "0x10de";

fn vendor_id(name: &str) -> Option<&'static str> {
    match name {
        "amd" => Some(VENDOR_AMD),
        "intel" => Some(VENDOR_INTEL),
        "nvidia" => Some(VENDOR_NVIDIA),
        _ => None,
    }
}

/// /dev/dri/renderD* of the first GPU of that vendor ("amd" | "intel").
/// A computer can have several GPUs (e.g. an Intel iGPU plus an Nvidia card), so the
/// vendor is looked up instead of blindly taking renderD128.
pub fn render_node(vendor: &str) -> Option<String> {
    render_node_in(vendor, std::path::Path::new(&std::env::var("MMT_DRM_DIR").unwrap_or_else(|_| "/sys/class/drm".into())))
}

pub fn render_node_in(vendor: &str, drm_dir: &std::path::Path) -> Option<String> {
    let want = vendor_id(vendor)?;
    let mut nodes: Vec<_> = std::fs::read_dir(drm_dir).ok()?.flatten().collect();
    nodes.sort_by_key(|e| e.file_name());
    for node in nodes {
        let name = node.file_name().to_string_lossy().to_string();
        if !name.starts_with("renderD") {
            continue;
        }
        if let Ok(v) = std::fs::read_to_string(node.path().join("device/vendor")) {
            if v.trim().eq_ignore_ascii_case(want) {
                return Some(format!("/dev/dri/{name}"));
            }
        }
    }
    None
}

/// Does this computer have the hardware H.264 encoder of the Raspberry Pi (Pi 3 / 4: /dev/video11)?
/// A Raspberry Pi 5 has none. `MMT_PI=1|0` forces the answer (for testing).
pub fn raspberry_pi_encoder() -> bool {
    if let Ok(v) = std::env::var("MMT_PI") {
        return v == "1";
    }
    if !cfg!(target_os = "linux") {
        return false;
    }
    let model = std::fs::read_to_string("/proc/device-tree/model").unwrap_or_default();
    model.contains("Raspberry Pi") && std::path::Path::new("/dev/video11").exists()
}

/// Vendors ("nvidia" | "amd" | "intel") of the display adapters found in /sys/bus/pci.
/// None = unknown (not Linux, or nothing found): then every codec is shown.
pub fn vendors_present() -> Option<HashSet<&'static str>> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let dir = std::env::var("MMT_PCI_DIR").unwrap_or_else(|_| "/sys/bus/pci/devices".into());
    let mut found = HashSet::new();
    for dev in std::fs::read_dir(dir).ok()?.flatten() {
        let class = std::fs::read_to_string(dev.path().join("class")).unwrap_or_default();
        if !class.trim().starts_with("0x03") {
            continue; // not a display adapter
        }
        let vendor = std::fs::read_to_string(dev.path().join("vendor")).unwrap_or_default();
        match vendor.trim().to_lowercase().as_str() {
            VENDOR_AMD => found.insert("amd"),
            VENDOR_INTEL => found.insert("intel"),
            VENDOR_NVIDIA => found.insert("nvidia"),
            _ => false,
        };
    }
    if found.is_empty() {
        None
    } else {
        Some(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_the_node_of_the_right_vendor() {
        let dir = std::env::temp_dir().join(format!("mmt-drm-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (node, vendor) in [("renderD128", "0x8086"), ("renderD129", "0x1002"), ("card0", "0x8086")] {
            fs::create_dir_all(dir.join(node).join("device")).unwrap();
            fs::write(dir.join(node).join("device/vendor"), format!("{vendor}\n")).unwrap();
        }
        assert_eq!(render_node_in("intel", &dir).as_deref(), Some("/dev/dri/renderD128"));
        assert_eq!(render_node_in("amd", &dir).as_deref(), Some("/dev/dri/renderD129"));
        assert_eq!(render_node_in("nvidia", &dir), None);
        assert_eq!(render_node_in("other", &dir), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
