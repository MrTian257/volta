use std::fmt::{self, Display};

use super::{
    check_fetched, check_shim_reachable, debug_already_fetched, info_fetched, info_installed,
    info_pinned, info_project_version, FetchStatus, Tool,
};
use crate::error::{ErrorKind, Fallible};
use crate::inventory::node_available;
use crate::session::Session;
use crate::style::{note_prefix, tool_version};
use crate::sync::VoltaLock;
use cfg_if::cfg_if;
use log::info;
use node_semver::Version;

mod fetch;
mod metadata;
mod resolve;
mod uninstall;

pub use fetch::load_default_npm_version;
pub use resolve::resolve;
pub use uninstall::uninstall;

// 根据不同的操作系统和架构组合定义相关常量
cfg_if! {
    if #[cfg(all(target_os = "windows", target_arch = "x86"))] {
        /// Node 发行版文件名中的操作系统组件
        pub const NODE_DISTRO_OS: &str = "win";
        /// Node 发行版文件名中的架构组件
        pub const NODE_DISTRO_ARCH: &str = "x86";
        /// Node 发行版文件的扩展名
        pub const NODE_DISTRO_EXTENSION: &str = "zip";
        /// Node 索引 `files` 数组中的文件标识符
        pub const NODE_DISTRO_IDENTIFIER: &str = "win-x86-zip";
    } else if #[cfg(all(target_os = "windows", target_arch = "x86_64"))] {
        /// Node 发行版文件名中的操作系统组件
        pub const NODE_DISTRO_OS: &str = "win";
        /// Node 发行版文件名中的架构组件
        pub const NODE_DISTRO_ARCH: &str = "x64";
        /// Node 发行版文件的扩展名
        pub const NODE_DISTRO_EXTENSION: &str = "zip";
        /// Node 索引 `files` 数组中的文件标识符
        pub const NODE_DISTRO_IDENTIFIER: &str = "win-x64-zip";
    } else if #[cfg(all(target_os = "windows", target_arch = "aarch64"))] {
        /// Node 发行版文件名中的操作系统组件
        pub const NODE_DISTRO_OS: &str = "win";
        /// Node 发行版文件名中的架构组件
        pub const NODE_DISTRO_ARCH: &str = "arm64";
        /// Node 发行版文件的扩展名
        pub const NODE_DISTRO_EXTENSION: &str = "zip";
        /// Node 索引 `files` 数组中的文件标识符
        pub const NODE_DISTRO_IDENTIFIER: &str = "win-arm64-zip";

        // 注意：Node 对 Windows ARM64 预构建二进制文件的支持从主版本 20 开始添加
        // 对于之前的版本，我们需要通过模拟器回退到 x64 二进制文件

        /// Node 发行版文件名中的回退架构组件
        pub const NODE_DISTRO_ARCH_FALLBACK: &str = "x64";
        /// Node 索引 `files` 数组中的回退文件标识符
        pub const NODE_DISTRO_IDENTIFIER_FALLBACK: &str = "win-x64-zip";
    } else if #[cfg(all(target_os = "macos", target_arch = "x86_64"))] {
        /// Node 发行版文件名中的操作系统组件
        pub const NODE_DISTRO_OS: &str = "darwin";
        /// Node 发行版文件名中的架构组件
        pub const NODE_DISTRO_ARCH: &str = "x64";
        /// Node 发行版文件的扩展名
        pub const NODE_DISTRO_EXTENSION: &str = "tar.gz";
        /// Node 索引 `files` 数组中的文件标识符
        pub const NODE_DISTRO_IDENTIFIER: &str = "osx-x64-tar";
    } else if #[cfg(all(target_os = "macos", target_arch = "aarch64"))] {
        /// Node 发行版文件名中的操作系统组件
        pub const NODE_DISTRO_OS: &str = "darwin";
        /// Node 发行版文件名中的架构组件
        pub const NODE_DISTRO_ARCH: &str = "arm64";
        /// Node 发行版文件的扩展名
        pub const NODE_DISTRO_EXTENSION: &str = "tar.gz";
        /// Node 索引 `files` 数组中的文件标识符
        pub const NODE_DISTRO_IDENTIFIER: &str = "osx-arm64-tar";

        // 注意：Node 对 Apple Silicon 预构建二进制文件的支持从主版本 16 开始添加
        // 对于之前的版本，我们需要通过 Rosetta 2 回退到 x64 二进制文件

        /// Node 发行版文件名中的回退架构组件
        pub const NODE_DISTRO_ARCH_FALLBACK: &str = "x64";
        /// Node 索引 `files` 数组中的回退文件标识符
        pub const NODE_DISTRO_IDENTIFIER_FALLBACK: &str = "osx-x64-tar";
    } else if #[cfg(all(target_os = "linux", target_arch = "x86_64"))] {
        /// Node 发行版文件名中的操作系统组件
        pub const NODE_DISTRO_OS: &str = "linux";
        /// Node 发行版文件名中的架构组件
        pub const NODE_DISTRO_ARCH: &str = "x64";
        /// Node 发行版文件的扩展名
        pub const NODE_DISTRO_EXTENSION: &str = "tar.gz";
        /// Node 索引 `files` 数组中的文件标识符
        pub const NODE_DISTRO_IDENTIFIER: &str = "linux-x64";
    } else if #[cfg(all(target_os = "linux", target_arch = "aarch64"))] {
        /// Node 发行版文件名中的操作系统组件
        pub const NODE_DISTRO_OS: &str = "linux";
        /// Node 发行版文件名中的架构组件
        pub const NODE_DISTRO_ARCH: &str = "arm64";
        /// Node 发行版文件的扩展名
        pub const NODE_DISTRO_EXTENSION: &str = "tar.gz";
        /// Node 索引 `files` 数组中的文件标识符
        pub const NODE_DISTRO_IDENTIFIER: &str = "linux-arm64";
    } else if #[cfg(all(target_os = "linux", target_arch = "arm"))] {
        /// Node 发行版文件名中的操作系统组件
        pub const NODE_DISTRO_OS: &str = "linux";
        /// Node 发行版文件名中的架构组件
        pub const NODE_DISTRO_ARCH: &str = "armv7l";
        /// Node 发行版文件的扩展名
        pub const NODE_DISTRO_EXTENSION: &str = "tar.gz";
        /// Node 索引 `files` 数组中的文件标识符
        pub const NODE_DISTRO_IDENTIFIER: &str = "linux-armv7l";
    } else {
        compile_error!("不支持的操作系统 + 架构组合");
    }
}

/// 完整的 Node 版本，不仅包括 Node 本身的版本，
/// 还包括与该 Node 安装一起全局安装的特定 npm 版本。
#[derive(Clone, Debug)]
pub struct NodeVersion {
    /// Node 本身的版本。
    pub runtime: Version,
    /// 与 Node 发行版一起全局安装的 npm 版本。
    pub npm: Version,
}

impl Display for NodeVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} (with {})",
            tool_version("node", &self.runtime),
            tool_version("npm", &self.npm)
        )
    }
}

/// 用于获取和安装 Node 的 Tool 实现
pub struct Node {
    pub(super) version: Version,
}

impl Node {
    pub fn new(version: Version) -> Self {
        Node { version }
    }

    // 为不需要回退的平台定义 archive_basename 方法
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "aarch64")
    )))]
    pub fn archive_basename(version: &Version) -> String {
        format!("node-v{}-{}-{}", version, NODE_DISTRO_OS, NODE_DISTRO_ARCH)
    }

    // 为 macOS ARM64 平台定义 archive_basename 方法
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub fn archive_basename(version: &Version) -> String {
        // 注意：Node 从主版本 16 开始为 Apple Silicon 提供预构建二进制文件
        // 在此之前，我们需要回退到 x64 二进制文件
        format!(
            "node-v{}-{}-{}",
            version,
            NODE_DISTRO_OS,
            if version.major >= 16 {
                NODE_DISTRO_ARCH
            } else {
                NODE_DISTRO_ARCH_FALLBACK
            }
        )
    }

    // 为 Windows ARM64 平台定义 archive_basename 方法
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    pub fn archive_basename(version: &Version) -> String {
        // 注意：Node 从主版本 20 开始为 Windows ARM 提供预构建二进制文件
        // 在此之前，我们需要回退到 x64 二进制文件
        format!(
            "node-v{}-{}-{}",
            version,
            NODE_DISTRO_OS,
            if version.major >= 20 {
                NODE_DISTRO_ARCH
            } else {
                NODE_DISTRO_ARCH_FALLBACK
            }
        )
    }

    pub fn archive_filename(version: &Version) -> String {
        format!(
            "{}.{}",
            Node::archive_basename(version),
            NODE_DISTRO_EXTENSION
        )
    }

    pub(crate) fn ensure_fetched(&self, session: &mut Session) -> Fallible<NodeVersion> {
        match check_fetched(|| node_available(&self.version))? {
            FetchStatus::AlreadyFetched => {
                debug_already_fetched(self);
                let npm = fetch::load_default_npm_version(&self.version)?;

                Ok(NodeVersion {
                    runtime: self.version.clone(),
                    npm,
                })
            }
            FetchStatus::FetchNeeded(_lock) => fetch::fetch(&self.version, session.hooks()?.node()),
        }
    }
}

impl Tool for Node {
    fn fetch(self: Box<Self>, session: &mut Session) -> Fallible<()> {
        let node_version = self.ensure_fetched(session)?;

        info_fetched(node_version);
        Ok(())
    }
    fn install(self: Box<Self>, session: &mut Session) -> Fallible<()> {
        // 如果可能，获取 Volta 目录的锁，以防止并发更改
        let _lock = VoltaLock::acquire();
        let node_version = self.ensure_fetched(session)?;

        let default_toolchain = session.toolchain_mut()?;
        default_toolchain.set_active_node(&self.version)?;

        // 如果用户有默认版本的 `npm`，我们不应该在成功消息中显示 "(with npm@X.Y.ZZZ)" 文本
        // 相反，我们应该检查捆绑版本是否高于默认版本，并通知用户
        // 注意：前面的行确保会有一个默认平台
        if let Some(default_npm) = &default_toolchain.platform().unwrap().npm {
            info_installed(&self); // 包括 node 版本

            if node_version.npm > *default_npm {
                info!(
                    "{} 此版本的 Node 包含 {}，它高于您的默认版本 ({})。
      要使用 Node 附带的版本，请运行 `volta install npm@bundled`",
                    note_prefix(),
                    tool_version("npm", node_version.npm),
                    default_npm.to_string()
                );
            }
        } else {
            info_installed(node_version); // 包括 node 和 npm 版本
        }

        check_shim_reachable("node");

        if let Ok(Some(project)) = session.project_platform() {
            info_project_version(tool_version("node", &project.node), &self);
        }

        Ok(())
    }
    fn pin(self: Box<Self>, session: &mut Session) -> Fallible<()> {
        if session.project()?.is_some() {
            let node_version = self.ensure_fetched(session)?;

            // 注意：我们知道这将成功，因为我们在上面检查过
            let project = session.project_mut()?.unwrap();
            project.pin_node(self.version.clone())?;

            // 如果用户有固定版本的 `npm`，我们不应该在成功消息中显示 "(with npm@X.Y.ZZZ)" 文本
            // 相反，我们应该检查捆绑版本是否高于固定版本，并通知用户
            // 注意：固定操作保证会有一个平台
            if let Some(pinned_npm) = &project.platform().unwrap().npm {
                info_pinned(self); // 包括 node 版本

                if node_version.npm > *pinned_npm {
                    info!(
                        "{} 此版本的 Node 包含 {}，它高于您的固定版本 ({})。
      要使用 Node 附带的版本，请运行 `volta pin npm@bundled`",
                        note_prefix(),
                        tool_version("npm", node_version.npm),
                        pinned_npm.to_string()
                    );
                }
            } else {
                info_pinned(node_version); // 包括 node 和 npm 版本
            }

            Ok(())
        } else {
            Err(ErrorKind::NotInPackage.into())
        }
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&tool_version("node", &self.version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_archive_basename() {
        assert_eq!(
            Node::archive_basename(&Version::parse("20.2.3").unwrap()),
            format!("node-v20.2.3-{}-{}", NODE_DISTRO_OS, NODE_DISTRO_ARCH)
        );
    }

    #[test]
    fn test_node_archive_filename() {
        assert_eq!(
            Node::archive_filename(&Version::parse("20.2.3").unwrap()),
            format!(
                "node-v20.2.3-{}-{}.{}",
                NODE_DISTRO_OS, NODE_DISTRO_ARCH, NODE_DISTRO_EXTENSION
            )
        );
    }

    #[test]
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn test_fallback_node_archive_basename() {
        assert_eq!(
            Node::archive_basename(&Version::parse("15.2.3").unwrap()),
            format!(
                "node-v15.2.3-{}-{}",
                NODE_DISTRO_OS, NODE_DISTRO_ARCH_FALLBACK
            )
        );
    }

    #[test]
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    fn test_fallback_node_archive_basename() {
        assert_eq!(
            Node::archive_basename(&Version::parse("19.2.3").unwrap()),
            format!(
                "node-v19.2.3-{}-{}",
                NODE_DISTRO_OS, NODE_DISTRO_ARCH_FALLBACK
            )
        );
    }

    #[test]
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn test_fallback_node_archive_filename() {
        assert_eq!(
            Node::archive_filename(&Version::parse("15.2.3").unwrap()),
            format!(
                "node-v15.2.3-{}-{}.{}",
                NODE_DISTRO_OS, NODE_DISTRO_ARCH_FALLBACK, NODE_DISTRO_EXTENSION
            )
        );
    }

    #[test]
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    fn test_fallback_node_archive_filename() {
        assert_eq!(
            Node::archive_filename(&Version::parse("19.2.3").unwrap()),
            format!(
                "node-v19.2.3-{}-{}.{}",
                NODE_DISTRO_OS, NODE_DISTRO_ARCH_FALLBACK, NODE_DISTRO_EXTENSION
            )
        );
    }
}
