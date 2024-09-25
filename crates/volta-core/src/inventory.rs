//! 提供用于处理 Volta 的 _库存_ 的类型，即本地可用工具版本的仓库。

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::Path;

use crate::error::{Context, ErrorKind, Fallible};
use crate::fs::read_dir_eager;
use crate::layout::volta_home;
use crate::tool::PackageConfig;
use crate::version::parse_version;
use log::debug;
use node_semver::Version;
use walkdir::WalkDir;

/// 检查给定的 Node 版本镜像是否在本地机器上可用
pub fn node_available(version: &Version) -> Fallible<bool> {
    volta_home().map(|home| {
        home.node_image_root_dir()
            .join(version.to_string())
            .exists()
    })
}

/// 收集本地机器上已获取的所有 Node 版本的集合
pub fn node_versions() -> Fallible<BTreeSet<Version>> {
    volta_home().and_then(|home| read_versions(home.node_image_root_dir()))
}

/// 检查给定的 npm 版本镜像是否在本地机器上可用
pub fn npm_available(version: &Version) -> Fallible<bool> {
    volta_home().map(|home| home.npm_image_dir(&version.to_string()).exists())
}

/// 收集本地机器上已获取的所有 npm 版本的集合
pub fn npm_versions() -> Fallible<BTreeSet<Version>> {
    volta_home().and_then(|home| read_versions(home.npm_image_root_dir()))
}

/// 检查给定的 pnpm 版本镜像是否在本地机器上可用
pub fn pnpm_available(version: &Version) -> Fallible<bool> {
    volta_home().map(|home| home.pnpm_image_dir(&version.to_string()).exists())
}

/// 收集本地机器上已获取的所有 pnpm 版本的集合
pub fn pnpm_versions() -> Fallible<BTreeSet<Version>> {
    volta_home().and_then(|home| read_versions(home.pnpm_image_root_dir()))
}

/// 检查给定的 Yarn 版本镜像是否在本地机器上可用
pub fn yarn_available(version: &Version) -> Fallible<bool> {
    volta_home().map(|home| home.yarn_image_dir(&version.to_string()).exists())
}

/// 收集本地机器上已获取的所有 Yarn 版本的集合
pub fn yarn_versions() -> Fallible<BTreeSet<Version>> {
    volta_home().and_then(|home| read_versions(home.yarn_image_root_dir()))
}

/// 收集本地机器上所有包配置的集合
pub fn package_configs() -> Fallible<BTreeSet<PackageConfig>> {
    let package_dir = volta_home()?.default_package_dir();

    WalkDir::new(package_dir)
        .max_depth(2)
        .into_iter()
        // 忽略任何未正确解析为 `DirEntry` 的项。
        // 对于这些项，我们无法做任何事情，也无法向用户报告任何错误。
        // 但是，在调试输出中记录失败。
        .filter_map(|entry| match entry {
            Ok(dir_entry) => {
                // 忽略目录条目和任何没有 .json 扩展名的文件。
                // 这将防止我们尝试将操作系统生成的文件解析为包配置
                // （例如 macOS 上的 `.DS_Store`）
                let extension = dir_entry.path().extension().and_then(OsStr::to_str);
                match (dir_entry.file_type().is_file(), extension) {
                    (true, Some(ext)) if ext.eq_ignore_ascii_case("json") => {
                        Some(dir_entry.into_path())
                    }
                    _ => None,
                }
            }
            Err(e) => {
                debug!("{}", e);
                None
            }
        })
        .map(PackageConfig::from_file)
        .collect()
}

/// 读取目录的内容并返回通过将目录名解析为语义版本找到的所有版本的集合
fn read_versions(dir: &Path) -> Fallible<BTreeSet<Version>> {
    let contents = read_dir_eager(dir).with_context(|| ErrorKind::ReadDirError {
        dir: dir.to_owned(),
    })?;

    Ok(contents
        .filter(|(_, metadata)| metadata.is_dir())
        .filter_map(|(entry, _)| parse_version(entry.file_name().to_string_lossy()).ok())
        .collect())
}
