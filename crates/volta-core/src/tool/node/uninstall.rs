use super::super::package::{BinConfig, PackageConfig};
use super::metadata::{RawNodeEntry, RawNodeIndex, NodeEntry};
use crate::error::{Context, ErrorKind, Fallible};
use crate::fs::{
    dir_entry_match, ok_if_not_found, read_dir_eager, remove_dir_if_exists, remove_file_if_exists,
};
use crate::layout::volta_home;
use crate::shim;
use crate::style::success_prefix;
use crate::sync::VoltaLock;
use log::{info, warn};
use crate::session::Session;
use crate::tool::node;
use crate::version::VersionSpec;



/// Uninstalls the specified package.
///
/// This removes:
///
/// - The JSON configuration files for both the package and its bins
/// - The shims for the package bins
/// - The package directory itself
pub fn uninstall(matching: VersionSpec, session: &mut Session) -> Fallible<()> {
    let name = "node";
    info!("node uninstall {}", matching);
    let home = volta_home()?;
    // Remove the package directory itself
    let version = node::resolve(matching, session)?;
    info!("test uninstall: {}" ,version);

    let node_image_dir = home.node_image_dir(&*version.to_string());

    info!("package_image_dir: {}", node_image_dir.to_str().unwrap().to_string());

    info!("{}" ,version);
    // remove_dir_if_exists(node_image_dir)?;

    // remove_shared_link_dir(name)?;

    // if package_found {
    //     info!("{} package '{}' uninstalled", success_prefix(), name);
    // } else {
    //     warn!("No package '{}' found to uninstall", name);
    // }

    Ok(())
}

/// Remove a shim and its associated configuration file
fn remove_config_and_shim(bin_name: &str, pkg_name: &str) -> Fallible<()> {
    shim::delete(bin_name)?;
    let config_file = volta_home()?.default_tool_bin_config(bin_name);
    remove_file_if_exists(config_file)?;
    info!(
        "Removed executable '{}' installed by '{}'",
        bin_name, pkg_name
    );
    Ok(())
}

/// Reads the contents of a directory and returns a Vec containing the names of
/// all the binaries installed by the given package.
fn binaries_from_package(package: &str) -> Fallible<Vec<String>> {
    let bin_config_dir = volta_home()?.default_bin_dir();

    dir_entry_match(bin_config_dir, |entry| {
        let path = entry.path();
        if let Ok(config) = BinConfig::from_file(path) {
            if config.package == package {
                return Some(config.name);
            }
        }
        None
    })
        .or_else(ok_if_not_found)
        .with_context(|| ErrorKind::ReadBinConfigDirError {
            dir: bin_config_dir.to_owned(),
        })
}

/// Remove the link to the package in the shared lib directory
///
/// For scoped packages, if the scope directory is now empty, it will also be removed
fn remove_shared_link_dir(name: &str) -> Fallible<()> {
    // Remove the link in the shared package directory, if it exists
    let mut shared_lib_dir = volta_home()?.shared_lib_dir(name);
    remove_dir_if_exists(&shared_lib_dir)?;

    // For scoped packages, clean up the scope directory if it is now empty
    if name.starts_with('@') {
        shared_lib_dir.pop();

        if let Ok(mut entries) = read_dir_eager(&shared_lib_dir) {
            if entries.next().is_none() {
                remove_dir_if_exists(&shared_lib_dir)?;
            }
        }
    }

    Ok(())
}
