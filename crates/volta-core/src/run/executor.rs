use std::collections::HashMap;
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
use std::process::{Command, ExitStatus};

use super::RECURSION_ENV_VAR;
use crate::command::create_command;
use crate::error::{Context, ErrorKind, Fallible};
use crate::layout::volta_home;
use crate::platform::{CliPlatform, Platform, System};
use crate::session::Session;
use crate::signal::pass_control_to_shim;
use crate::style::{note_prefix, tool_version};
use crate::sync::VoltaLock;
use crate::tool::package::{DirectInstall, InPlaceUpgrade, PackageConfig, PackageManager};
use crate::tool::Spec;
use log::{info, warn};

// 定义Executor枚举，表示不同类型的执行器
pub enum Executor {
    Tool(Box<ToolCommand>),
    PackageInstall(Box<PackageInstallCommand>),
    PackageLink(Box<PackageLinkCommand>),
    PackageUpgrade(Box<PackageUpgradeCommand>),
    InternalInstall(Box<InternalInstallCommand>),
    Uninstall(Box<UninstallCommand>),
    Multiple(Vec<Executor>),
}

impl Executor {
    // 为执行器设置环境变量
    pub fn envs<K, V, S>(&mut self, envs: &HashMap<K, V, S>)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        match self {
            Executor::Tool(cmd) => cmd.envs(envs),
            Executor::PackageInstall(cmd) => cmd.envs(envs),
            Executor::PackageLink(cmd) => cmd.envs(envs),
            Executor::PackageUpgrade(cmd) => cmd.envs(envs),
            // 内部安装和卸载不依赖环境变量
            Executor::InternalInstall(_) => {}
            Executor::Uninstall(_) => {}
            Executor::Multiple(executors) => {
                for exe in executors {
                    exe.envs(envs);
                }
            }
        }
    }

    // 设置命令行平台
    pub fn cli_platform(&mut self, cli: CliPlatform) {
        match self {
            Executor::Tool(cmd) => cmd.cli_platform(cli),
            Executor::PackageInstall(cmd) => cmd.cli_platform(cli),
            Executor::PackageLink(cmd) => cmd.cli_platform(cli),
            Executor::PackageUpgrade(cmd) => cmd.cli_platform(cli),
            // 内部安装和卸载不依赖Node平台
            Executor::InternalInstall(_) => {}
            Executor::Uninstall(_) => {}
            Executor::Multiple(executors) => {
                for exe in executors {
                    exe.cli_platform(cli.clone());
                }
            }
        }
    }

    // 执行命令
    pub fn execute(self, session: &mut Session) -> Fallible<ExitStatus> {
        match self {
            Executor::Tool(cmd) => cmd.execute(session),
            Executor::PackageInstall(cmd) => cmd.execute(session),
            Executor::PackageLink(cmd) => cmd.execute(session),
            Executor::PackageUpgrade(cmd) => cmd.execute(session),
            Executor::InternalInstall(cmd) => cmd.execute(session),
            Executor::Uninstall(cmd) => cmd.execute(session),
            Executor::Multiple(executors) => {
                info!(
                    "{} Volta is processing each package separately",
                    note_prefix()
                );
                for exe in executors {
                    let status = exe.execute(session)?;
                    // 如果任何子命令失败，停止安装并返回失败状态
                    if !status.success() {
                        return Ok(status);
                    }
                }
                // 所有子命令成功，返回成功状态
                Ok(ExitStatus::from_raw(0))
            }
        }
    }
}

// 从Vec<Executor>转换为Executor
impl From<Vec<Executor>> for Executor {
    fn from(mut executors: Vec<Executor>) -> Self {
        if executors.len() == 1 {
            executors.pop().unwrap()
        } else {
            Executor::Multiple(executors)
        }
    }
}

// 用于启动Volta管理的工具的进程构建器
pub struct ToolCommand {
    command: Command,
    platform: Option<Platform>,
    kind: ToolKind,
}

// 定义工具类型枚举
pub enum ToolKind {
    Node,
    Npm,
    Npx,
    Pnpm,
    Yarn,
    ProjectLocalBinary(String),
    DefaultBinary(String),
    Bypass(String),
}

impl ToolCommand {
    // 创建新的ToolCommand实例
    pub fn new<E, A, S>(exe: E, args: A, platform: Option<Platform>, kind: ToolKind) -> Self
    where
        E: AsRef<OsStr>,
        A: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = create_command(exe);
        command.args(args);

        Self {
            command,
            platform,
            kind,
        }
    }

    // 添加或更新命令将使用的环境变量
    pub fn envs<E, K, V>(&mut self, envs: E)
    where
        E: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.envs(envs);
    }

    // 添加或更新单个环境变量
    pub fn env<K, V>(&mut self, key: K, value: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.env(key, value);
    }

    // 更新命令的Platform以包含命令行值
    pub fn cli_platform(&mut self, cli: CliPlatform) {
        self.platform = match self.platform.take() {
            Some(base) => Some(cli.merge(base)),
            None => cli.into(),
        };
    }

    // 运行命令，如果成功启动则返回ExitStatus
    pub fn execute(mut self, session: &mut Session) -> Fallible<ExitStatus> {
        let (path, on_failure) = match self.kind {
            ToolKind::Node => super::node::execution_context(self.platform, session)?,
            ToolKind::Npm => super::npm::execution_context(self.platform, session)?,
            ToolKind::Npx => super::npx::execution_context(self.platform, session)?,
            ToolKind::Pnpm => super::pnpm::execution_context(self.platform, session)?,
            ToolKind::Yarn => super::yarn::execution_context(self.platform, session)?,
            ToolKind::DefaultBinary(bin) => {
                super::binary::default_execution_context(bin, self.platform, session)?
            }
            ToolKind::ProjectLocalBinary(bin) => {
                super::binary::local_execution_context(bin, self.platform, session)?
            }
            ToolKind::Bypass(command) => (System::path()?, ErrorKind::BypassError { command }),
        };

        self.command.env(RECURSION_ENV_VAR, "1");
        self.command.env("PATH", path);

        pass_control_to_shim();
        self.command.status().with_context(|| on_failure)
    }
}

// 将ToolCommand转换为Executor
impl From<ToolCommand> for Executor {
    fn from(cmd: ToolCommand) -> Self {
        Executor::Tool(Box::new(cmd))
    }
}

// 用于启动包安装命令的进程构建器
pub struct PackageInstallCommand {
    command: Command,
    installer: DirectInstall,
    platform: Platform,
}

impl PackageInstallCommand {
    // 创建新的PackageInstallCommand实例
    pub fn new<A, S>(args: A, platform: Platform, manager: PackageManager) -> Fallible<Self>
    where
        A: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let installer = DirectInstall::new(manager)?;

        let mut command = match manager {
            PackageManager::Npm => create_command("npm"),
            PackageManager::Pnpm => create_command("pnpm"),
            PackageManager::Yarn => create_command("yarn"),
        };
        command.args(args);

        Ok(PackageInstallCommand {
            command,
            installer,
            platform,
        })
    }

    // 创建用于npm link的PackageInstallCommand实例
    pub fn for_npm_link<A, S>(args: A, platform: Platform, name: String) -> Fallible<Self>
    where
        A: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let installer = DirectInstall::with_name(PackageManager::Npm, name)?;

        let mut command = create_command("npm");
        command.args(args);

        Ok(PackageInstallCommand {
            command,
            installer,
            platform,
        })
    }

    // 添加或更新命令将使用的环境变量
    pub fn envs<E, K, V>(&mut self, envs: E)
    where
        E: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.envs(envs);
    }

    // 更新命令的Platform以包含命令行值
    pub fn cli_platform(&mut self, cli: CliPlatform) {
        self.platform = cli.merge(self.platform.clone());
    }

    // 运行安装命令，应用必要的修改以安装到Volta数据目录
    pub fn execute(mut self, session: &mut Session) -> Fallible<ExitStatus> {
        let _lock = VoltaLock::acquire();
        let image = self.platform.checkout(session)?;
        let path = image.path()?;

        self.command.env(RECURSION_ENV_VAR, "1");
        self.command.env("PATH", path);
        self.installer.setup_command(&mut self.command);

        let status = self
            .command
            .status()
            .with_context(|| ErrorKind::BinaryExecError)?;

        if status.success() {
            self.installer.complete_install(&image)?;
        }

        Ok(status)
    }
}

// 将PackageInstallCommand转换为Executor
impl From<PackageInstallCommand> for Executor {
    fn from(cmd: PackageInstallCommand) -> Self {
        Executor::PackageInstall(Box::new(cmd))
    }
}

// 用于启动`npm link <package>`命令的进程构建器
pub struct PackageLinkCommand {
    command: Command,
    tool: String,
    platform: Platform,
}

impl PackageLinkCommand {
    // 创建新的PackageLinkCommand实例
    pub fn new<A, S>(args: A, platform: Platform, tool: String) -> Self
    where
        A: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = create_command("npm");
        command.args(args);

        PackageLinkCommand {
            command,
            tool,
            platform,
        }
    }

    // 添加或更新命令将使用的环境变量
    pub fn envs<E, K, V>(&mut self, envs: E)
    where
        E: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.envs(envs);
    }

    // 更新命令的Platform以包含命令行值
    pub fn cli_platform(&mut self, cli: CliPlatform) {
        self.platform = cli.merge(self.platform.clone());
    }

    // 运行link命令，应用必要的修改以从Volta数据目录中提取
    pub fn execute(mut self, session: &mut Session) -> Fallible<ExitStatus> {
        self.check_linked_package(session)?;

        let image = self.platform.checkout(session)?;
        let path = image.path()?;

        self.command.env(RECURSION_ENV_VAR, "1");
        self.command.env("PATH", path);
        let package_root = volta_home()?.package_image_dir(&self.tool);
        PackageManager::Npm.setup_global_command(&mut self.command, package_root);

        self.command
            .status()
            .with_context(|| ErrorKind::BinaryExecError)
    }

    // 检查链接包的可能失败情况
    fn check_linked_package(&self, session: &mut Session) -> Fallible<()> {
        let config =
            PackageConfig::from_file(volta_home()?.default_package_config_file(&self.tool))
                .with_context(|| ErrorKind::NpmLinkMissingPackage {
                    package: self.tool.clone(),
                })?;

        if config.manager != PackageManager::Npm {
            return Err(ErrorKind::NpmLinkWrongManager {
                package: self.tool.clone(),
            }
            .into());
        }

        if let Some(platform) = session.project_platform()? {
            if platform.node.major != config.platform.node.major {
                warn!(
                    "the current project is using {}, but package '{}' was linked using {}. These might not interact correctly.",
                    tool_version("node", &platform.node),
                    self.tool,
                    tool_version("node", &config.platform.node)
                );
            }
        }

        Ok(())
    }
}

// 将PackageLinkCommand转换为Executor
impl From<PackageLinkCommand> for Executor {
    fn from(cmd: PackageLinkCommand) -> Self {
        Executor::PackageLink(Box::new(cmd))
    }
}

// 用于启动全局包升级命令的进程构建器
pub struct PackageUpgradeCommand {
    command: Command,
    upgrader: InPlaceUpgrade,
    platform: Platform,
}

impl PackageUpgradeCommand {
    // 创建新的PackageUpgradeCommand实例
    pub fn new<A, S>(
        args: A,
        package: String,
        platform: Platform,
        manager: PackageManager,
    ) -> Fallible<Self>
    where
        A: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let upgrader = InPlaceUpgrade::new(package, manager)?;

        let mut command = match manager {
            PackageManager::Npm => create_command("npm"),
            PackageManager::Pnpm => create_command("pnpm"),
            PackageManager::Yarn => create_command("yarn"),
        };
        command.args(args);

        Ok(PackageUpgradeCommand {
            command,
            upgrader,
            platform,
        })
    }

    // 添加或更新命令将使用的环境变量
    pub fn envs<E, K, V>(&mut self, envs: E)
    where
        E: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.envs(envs);
    }

    // 更新命令的Platform以包含命令行值
    pub fn cli_platform(&mut self, cli: CliPlatform) {
        self.platform = cli.merge(self.platform.clone());
    }

    // 运行升级命令，应用必要的修改以指向Volta镜像目录
    pub fn execute(mut self, session: &mut Session) -> Fallible<ExitStatus> {
        self.upgrader.check_upgraded_package()?;

        let _lock = VoltaLock::acquire();
        let image = self.platform.checkout(session)?;
        let path = image.path()?;

        self.command.env(RECURSION_ENV_VAR, "1");
        self.command.env("PATH", path);
        self.upgrader.setup_command(&mut self.command);

        let status = self
            .command
            .status()
            .with_context(|| ErrorKind::BinaryExecError)?;

        if status.success() {
            self.upgrader.complete_upgrade(&image)?;
        }

        Ok(status)
    }
}

// 将PackageUpgradeCommand转换为Executor
impl From<PackageUpgradeCommand> for Executor {
    fn from(cmd: PackageUpgradeCommand) -> Self {
        Executor::PackageUpgrade(Box::new(cmd))
    }
}

// 用于运行内部安装的执行器
pub struct InternalInstallCommand {
    tool: Spec,
}

impl InternalInstallCommand {
    // 创建新的InternalInstallCommand实例
    pub fn new(tool: Spec) -> Self {
        InternalInstallCommand { tool }
    }

    // 使用Volta的内部安装逻辑运行安装
    fn execute(self, session: &mut Session) -> Fallible<ExitStatus> {
        info!(
            "{} using Volta to install {}",
            note_prefix(),
            self.tool.name()
        );

        self.tool.resolve(session)?.install(session)?;

        Ok(ExitStatus::from_raw(0))
    }
}

// 将InternalInstallCommand转换为Executor
impl From<InternalInstallCommand> for Executor {
    fn from(cmd: InternalInstallCommand) -> Self {
        Executor::InternalInstall(Box::new(cmd))
    }
}

// 用于运行工具卸载命令的执行器
pub struct UninstallCommand {
    tool: Spec,
}

impl UninstallCommand {
    // 创建新的UninstallCommand实例
    pub fn new(tool: Spec) -> Self {
        UninstallCommand { tool }
    }

    // 使用Volta的内部卸载逻辑运行卸载
    fn execute(self, session: &mut Session) -> Fallible<ExitStatus> {
        info!(
            "{} using Volta to uninstall {}",
            note_prefix(),
            self.tool.name()
        );

        self.tool.uninstall(session)?;

        Ok(ExitStatus::from_raw(0))
    }
}

// 将UninstallCommand转换为Executor
impl From<UninstallCommand> for Executor {
    fn from(cmd: UninstallCommand) -> Self {
        Executor::Uninstall(Box::new(cmd))
    }
}
