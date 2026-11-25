//! Choosing the Vulkan drivers the loader offers.

#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
const DISABLE_VARIABLE: &str = "VK_LOADER_DRIVERS_DISABLE";

/// The part of the software driver's manifest names that identifies it.
#[cfg(target_os = "linux")]
const SOFTWARE_NAME: &str = "lvp";

#[cfg(target_os = "linux")]
const SOFTWARE_DRIVER: &str = "*lvp*";

/// The variables that select the loader's drivers.
#[cfg(target_os = "linux")]
const DRIVER_VARIABLES: [&str; 5] = [
    "VK_LOADER_DRIVERS_SELECT",
    "VK_LOADER_DRIVERS_DISABLE",
    "VK_DRIVER_FILES",
    "VK_ICD_FILENAMES",
    "VK_ADD_DRIVER_FILES",
];

/// Excludes the software Vulkan driver from the loader when a hardware driver is installed.
///
/// The driver is excluded only in a process with one thread, and only when no driver selection
/// variable is set.
///
/// Returns whether the driver was excluded.
// Setting an environment variable is `unsafe` in edition 2024.
#[allow(unsafe_code)]
pub fn prefer_hardware_vulkan() -> bool {
    #[cfg(target_os = "linux")]
    {
        let lookup = |name: &str| std::env::var_os(name);
        if !wanted(lookup, &search_directories(lookup)) || thread_count() != Some(1) {
            return false;
        }
        // SAFETY: the process has one thread, checked immediately above, so no other thread can
        // read or write the environment at the same time.
        unsafe { std::env::set_var(DISABLE_VARIABLE, SOFTWARE_DRIVER) };
        tracing::debug!("the software Vulkan driver is excluded: a hardware driver is installed");
        true
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Reports whether no variable of `lookup` selects drivers, and one of `directories` holds the
/// manifest of a driver other than the software one.
#[cfg(target_os = "linux")]
fn wanted(lookup: impl Fn(&str) -> Option<std::ffi::OsString>, directories: &[PathBuf]) -> bool {
    if DRIVER_VARIABLES.iter().any(|name| lookup(name).is_some()) {
        return false;
    }
    directories
        .iter()
        .any(|directory| has_other_driver(directory))
}

/// Reports whether `directory` holds the manifest of a driver other than the software one.
#[cfg(target_os = "linux")]
fn has_other_driver(directory: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        name.ends_with(".json") && !name.contains(SOFTWARE_NAME)
    })
}

/// Returns the directories the loader searches for driver manifests, with the variables of
/// `lookup` and their defaults.
#[cfg(target_os = "linux")]
fn search_directories(lookup: impl Fn(&str) -> Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let home = lookup("HOME").map(PathBuf::from);
    let single = |name: &str, fallback: &str| {
        lookup(name)
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join(fallback)))
    };
    let list = |name: &str, fallback: &str| {
        let value = lookup(name).unwrap_or_else(|| fallback.into());
        std::env::split_paths(&value).collect::<Vec<_>>()
    };
    let mut bases = Vec::new();
    bases.extend(single("XDG_CONFIG_HOME", ".config"));
    bases.extend(list("XDG_CONFIG_DIRS", "/etc/xdg"));
    bases.push(PathBuf::from("/usr/local/etc"));
    bases.push(PathBuf::from("/etc"));
    bases.extend(single("XDG_DATA_HOME", ".local/share"));
    bases.extend(list("XDG_DATA_DIRS", "/usr/local/share:/usr/share"));
    bases
        .into_iter()
        .map(|base| base.join("vulkan").join("icd.d"))
        .collect()
}

/// Returns how many threads the process has, if `/proc` reports it.
#[cfg(target_os = "linux")]
fn thread_count() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Threads:"))
        .and_then(|count| count.trim().parse().ok())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{DISABLE_VARIABLE, prefer_hardware_vulkan, search_directories, wanted};

    #[test]
    fn a_process_with_several_threads_keeps_its_environment() {
        // The test harness runs each test on a thread of its own, so this process has several.
        let before = std::env::var_os(DISABLE_VARIABLE);
        assert!(!prefer_hardware_vulkan());
        assert_eq!(std::env::var_os(DISABLE_VARIABLE), before);
    }

    fn manifests(test: &str, names: &[&str]) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("iceblink-loader-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("the test directory can be created");
        for name in names {
            std::fs::write(directory.join(name), "{}").expect("the manifest can be written");
        }
        directory
    }

    fn unset(_: &str) -> Option<OsString> {
        None
    }

    #[test]
    fn a_hardware_manifest_beside_lavapipe_excludes_lavapipe() {
        let directory = manifests("both", &["lvp_icd.x86_64.json", "nvidia_icd.json"]);
        assert!(wanted(unset, &[directory]));
    }

    #[test]
    fn lavapipe_alone_is_kept() {
        let directory = manifests("alone", &["lvp_icd.x86_64.json", "lvp_icd.i686.json"]);
        assert!(!wanted(unset, &[directory]));
    }

    #[test]
    fn files_other_than_manifests_do_not_count() {
        let directory = manifests("other", &["lvp_icd.x86_64.json", "README"]);
        assert!(!wanted(unset, &[directory]));
    }

    #[test]
    fn a_driver_variable_set_by_the_user_wins() {
        let directory = manifests("chosen", &["lvp_icd.x86_64.json", "radeon_icd.x86_64.json"]);
        let chosen = |name: &str| (name == "VK_DRIVER_FILES").then(|| OsString::from("/x.json"));
        assert!(!wanted(chosen, &[directory]));
    }

    #[test]
    fn missing_directories_mean_no_driver() {
        assert!(!wanted(
            unset,
            &[PathBuf::from("/nonexistent/vulkan/icd.d")]
        ));
    }

    #[test]
    fn the_search_follows_the_xdg_variables_and_their_defaults() {
        let lookup = |name: &str| match name {
            "HOME" => Some(OsString::from("/home/user")),
            "XDG_DATA_DIRS" => Some(OsString::from("/a:/b")),
            _ => None,
        };
        let directories = search_directories(lookup);
        for expected in [
            "/home/user/.config/vulkan/icd.d",
            "/etc/xdg/vulkan/icd.d",
            "/etc/vulkan/icd.d",
            "/home/user/.local/share/vulkan/icd.d",
            "/a/vulkan/icd.d",
            "/b/vulkan/icd.d",
        ] {
            assert!(
                directories.contains(&PathBuf::from(expected)),
                "{expected} is searched: {directories:?}"
            );
        }
        assert!(!directories.contains(&PathBuf::from("/usr/share/vulkan/icd.d")));
    }
}
