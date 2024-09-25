use super::super::package::{BinConfig, PackageConfig};
use super::metadata::{NodeEntry, RawNodeEntry, RawNodeIndex};
use crate::error::{Context, ErrorKind, Fallible};
use crate::fs::{
    dir_entry_match, ok_if_not_found, read_dir_eager, remove_dir_if_exists, remove_file_if_exists,
};
use crate::layout::volta_home;
use crate::session::Session;
use crate::shim;
use crate::style::success_prefix;
use crate::sync::VoltaLock;
use crate::tool::node;
use crate::version::VersionSpec;
use log::{info, warn};

/// 卸载指定的包。
///
/// 这将移除：
///
/// - 包及其二进制文件的 JSON 配置文件
/// - 包二进制文件的 shim
/// - 包目录本身
pub fn uninstall(matching: VersionSpec, session: &mut Session) -> Fallible<()> {
    let name = "node";
    info!("node 卸载 {}", matching);
    let home = volta_home()?;
    // 移除包目录本身
    let version = node::resolve(matching, session)?;
    info!("测试卸载: {}", version);

    let node_image_dir = home.node_image_dir(&*version.to_string());

    info!(
        "包镜像目录: {}",
        node_image_dir.to_str().unwrap().to_string()
    );

    info!("{}", version);
    // remove_dir_if_exists(node_image_dir)?;

    // remove_shared_link_dir(name)?;

    // if package_found {
    //     info!("{} 包 '{}' 已卸载", success_prefix(), name);
    // } else {
    //     warn!("未找到要卸载的包 '{}'", name);
    // }

    Ok(())
}

/// 移除 shim 及其关联的配置文件
fn remove_config_and_shim(bin_name: &str, pkg_name: &str) -> Fallible<()> {
    shim::delete(bin_name)?;
    let config_file = volta_home()?.default_tool_bin_config(bin_name);
    remove_file_if_exists(config_file)?;
    info!("已移除由 '{}' 安装的可执行文件 '{}'", pkg_name, bin_name);
    Ok(())
}

/// 读取目录内容并返回一个 Vec，包含给定包安装的所有二进制文件的名称。
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

/// 移除共享库目录中指向包的链接
///
/// 对于作用域包，如果作用域目录现在为空，它也将被移除
fn remove_shared_link_dir(name: &str) -> Fallible<()> {
    // 移除共享包目录中的链接（如果存在）
    let mut shared_lib_dir = volta_home()?.shared_lib_dir(name);
    remove_dir_if_exists(&shared_lib_dir)?;

    // 对于作用域包，如果作用域目录现在为空，则清理它
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
