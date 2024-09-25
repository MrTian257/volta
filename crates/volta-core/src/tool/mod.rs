use std::env;
use std::fmt::{self, Display};
use std::path::PathBuf;

use crate::error::{ErrorKind, Fallible};
use crate::layout::volta_home;
use crate::session::Session;
use crate::style::{note_prefix, success_prefix, tool_version};
use crate::sync::VoltaLock;
use crate::version::VersionSpec;
use crate::VOLTA_FEATURE_PNPM;
use cfg_if::cfg_if;
use log::{debug, info};

// 导入各种工具模块
pub mod node;
pub mod npm;
pub mod package;
pub mod pnpm;
mod registry;
mod serial;
pub mod yarn;

// 从各个模块中重新导出一些类型和函数
use crate::tool::package::uninstall;
pub use node::{
    load_default_npm_version, Node, NODE_DISTRO_ARCH, NODE_DISTRO_EXTENSION, NODE_DISTRO_OS,
};
pub use npm::{BundledNpm, Npm};
pub use package::{BinConfig, Package, PackageConfig, PackageManifest};
pub use pnpm::Pnpm;
pub use registry::PackageDetails;
pub use yarn::Yarn;

// 调试日志：工具已经被获取，跳过下载
fn debug_already_fetched<T: Display>(tool: T) {
    debug!("{} has already been fetched, skipping download", tool);
}

// 信息日志：工具已安装并设置为默认
fn info_installed<T: Display>(tool: T) {
    info!("{} installed and set {tool} as default", success_prefix());
}

// 信息日志：工具已获取
fn info_fetched<T: Display>(tool: T) {
    info!("{} fetched {tool}", success_prefix());
}

// 信息日志：工具已在 package.json 中固定
fn info_pinned<T: Display>(tool: T) {
    info!("{} pinned {tool} in package.json", success_prefix());
}

// 信息日志：项目版本和默认版本的对比
fn info_project_version<P, D>(project_version: P, default_version: D)
where
    P: Display,
    D: Display,
{
    info!(
        r#"{} you are using {project_version} in the current project; to
         instead use {default_version}, run `volta pin {default_version}`"#,
        note_prefix()
    );
}

/// 表示可以对工具执行的所有操作的特征
pub trait Tool: Display {
    /// 将工具获取到本地库存中
    fn fetch(self: Box<Self>, session: &mut Session) -> Fallible<()>;
    /// 安装工具，使其成为默认工具，在用户机器的任何地方都可用
    fn install(self: Box<Self>, session: &mut Session) -> Fallible<()>;
    /// 在本地项目中固定工具，使其在项目中可用
    fn pin(self: Box<Self>, session: &mut Session) -> Fallible<()>;
}

/// 工具及其关联版本的规范
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum Spec {
    Node(VersionSpec),
    Npm(VersionSpec),
    Pnpm(VersionSpec),
    Yarn(VersionSpec),
    Package(String, VersionSpec),
}

impl Spec {
    /// 将工具规范解析为可以获取的完全实现的工具
    pub fn resolve(self, session: &mut Session) -> Fallible<Box<dyn Tool>> {
        match self {
            Spec::Node(version) => {
                let version = node::resolve(version, session)?;
                Ok(Box::new(Node::new(version)))
            }
            Spec::Npm(version) => match npm::resolve(version, session)? {
                Some(version) => Ok(Box::new(Npm::new(version))),
                None => Ok(Box::new(BundledNpm)),
            },
            Spec::Pnpm(version) => {
                // 如果设置了 pnpm 功能标志，使用特殊的包管理器逻辑来处理 pnpm 的解析（最终获取/安装）
                // 如果没有设置，则回退到全局包行为，这是在添加 pnpm 支持之前的情况
                if env::var_os(VOLTA_FEATURE_PNPM).is_some() {
                    let version = pnpm::resolve(version, session)?;
                    Ok(Box::new(Pnpm::new(version)))
                } else {
                    let package = Package::new("pnpm".to_owned(), version)?;
                    Ok(Box::new(package))
                }
            }
            Spec::Yarn(version) => {
                let version = yarn::resolve(version, session)?;
                Ok(Box::new(Yarn::new(version)))
            }
            // 使用全局包安装时，我们允许包管理器执行版本解析
            Spec::Package(name, version) => {
                let package = Package::new(name, version)?;
                Ok(Box::new(package))
            }
        }
    }

    /// 卸载工具，从本地库存中移除它
    ///
    /// 这在 Spec 上实现，而不是在 Resolved 上实现，因为目前在卸载工具之前不需要解析特定版本。
    pub fn uninstall(self, session: &mut Session) -> Fallible<()> {
        match self {
            Spec::Node(var) => node::uninstall(var, session),
            Spec::Npm(_) => Err(ErrorKind::Unimplemented {
                feature: "Uninstalling npm".into(),
            }
            .into()),
            Spec::Pnpm(_) => {
                if env::var_os(VOLTA_FEATURE_PNPM).is_some() {
                    Err(ErrorKind::Unimplemented {
                        feature: "Uninstalling pnpm".into(),
                    }
                    .into())
                } else {
                    package::uninstall("pnpm")
                }
            }
            Spec::Yarn(_) => Err(ErrorKind::Unimplemented {
                feature: "Uninstalling yarn".into(),
            }
            .into()),
            Spec::Package(name, _) => package::uninstall(&name),
        }
    }

    /// 工具的名称，不包括版本，用于消息传递
    pub fn name(&self) -> &str {
        match self {
            Spec::Node(_) => "Node",
            Spec::Npm(_) => "npm",
            Spec::Pnpm(_) => "pnpm",
            Spec::Yarn(_) => "Yarn",
            Spec::Package(name, _) => name,
        }
    }
}

impl Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Spec::Node(ref version) => tool_version("node", version),
            Spec::Npm(ref version) => tool_version("npm", version),
            Spec::Pnpm(ref version) => tool_version("pnpm", version),
            Spec::Yarn(ref version) => tool_version("yarn", version),
            Spec::Package(ref name, ref version) => tool_version(name, version),
        };
        f.write_str(&s)
    }
}

/// 表示检查工具是否在本地可用的结果
///
/// 如果需要获取，将尽可能包含 Volta 目录的独占锁
enum FetchStatus {
    AlreadyFetched,
    FetchNeeded(Option<VoltaLock>),
}

/// 使用提供的 `already_fetched` 谓词来确定工具是否可用
///
/// 这使用双重检查逻辑，以正确处理并发获取请求：
///
/// - 如果 `already_fetched` 表明需要获取，我们获取 Volta 目录的独占锁
/// - 然后，我们再次检查，以确认没有其他进程在我们等待锁时完成了获取
///
/// 注意：如果获取锁失败，我们仍然继续，因为获取仍然是必要的。
fn check_fetched<F>(already_fetched: F) -> Fallible<FetchStatus>
where
    F: Fn() -> Fallible<bool>,
{
    if !already_fetched()? {
        let lock = match VoltaLock::acquire() {
            Ok(l) => Some(l),
            Err(_) => {
                debug!("Unable to acquire lock on Volta directory!");
                None
            }
        };

        if !already_fetched()? {
            Ok(FetchStatus::FetchNeeded(lock))
        } else {
            Ok(FetchStatus::AlreadyFetched)
        }
    } else {
        Ok(FetchStatus::AlreadyFetched)
    }
}

fn download_tool_error(tool: Spec, from_url: impl AsRef<str>) -> impl FnOnce() -> ErrorKind {
    let from_url = from_url.as_ref().to_string();
    || ErrorKind::DownloadToolNetworkError { tool, from_url }
}

fn registry_fetch_error(
    tool: impl AsRef<str>,
    from_url: impl AsRef<str>,
) -> impl FnOnce() -> ErrorKind {
    let tool = tool.as_ref().to_string();
    let from_url = from_url.as_ref().to_string();
    || ErrorKind::RegistryFetchError { tool, from_url }
}

cfg_if!(
    if #[cfg(windows)] {
        const PATH_VAR_NAME: &str = "Path";
    } else {
        const PATH_VAR_NAME: &str = "PATH";
    }
);

/// 检查新安装的 shim 是否在 PATH 中排在第一位。如果不是，我们想通知用户
/// 他们需要将其移到 PATH 的开头，以确保一切按预期工作。
pub fn check_shim_reachable(shim_name: &str) {
    let Some(expected_dir) = find_expected_shim_dir(shim_name) else {
        return;
    };

    let Ok(resolved) = which::which(shim_name) else {
        info!(
            "{} cannot find command {}. Please ensure that {} is available on your {}.",
            note_prefix(),
            shim_name,
            expected_dir.display(),
            PATH_VAR_NAME,
        );
        return;
    };

    if !resolved.starts_with(&expected_dir) {
        info!(
            "{} {} is shadowed by another binary of the same name at {}. To ensure your commands work as expected, please move {} to the start of your {}.",
            note_prefix(),
            shim_name,
            resolved.display(),
            expected_dir.display(),
            PATH_VAR_NAME
        );
    }
}

/// 在 Volta 目录中定位相关 shim 的基本目录。
///
/// 在 Unix 上，所有的 shim，包括默认的 shim，都安装在 `VoltaHome::shim_dir` 中
#[cfg(unix)]
fn find_expected_shim_dir(_shim_name: &str) -> Option<PathBuf> {
    volta_home().ok().map(|home| home.shim_dir().to_owned())
}

/// 在 Volta 目录中定位相关 shim 的基本目录。
///
/// 在 Windows 上，默认的 shim（node、npm、yarn 等）与 Volta 二进制文件一起安装在 `Program Files` 中。
/// 为了确定我们应该检查的位置，我们首先在 `VoltaHome::shim_dir` 中查找相关的 shim。
/// 如果它在那里，我们使用那个目录。如果不在，我们假设它必须是一个默认的 shim，
/// 并返回 `VoltaInstall::root`，这是 Volta 本身安装的位置。
#[cfg(windows)]
fn find_expected_shim_dir(shim_name: &str) -> Option<PathBuf> {
    use crate::layout::volta_install;

    let home = volta_home().ok()?;

    if home.shim_file(shim_name).exists() {
        Some(home.shim_dir().to_owned())
    } else {
        volta_install()
            .ok()
            .map(|install| install.root().to_owned())
    }
}
