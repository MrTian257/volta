//! 提供 `Session` 类型，表示用户在执行 Volta 工具期间的状态，
//! 包括他们的当前目录、Volta 钩子配置和本地库存的状态。

use std::fmt::{self, Display, Formatter};
use std::process::exit;

use crate::error::{ExitCode, Fallible, VoltaError};
use crate::event::EventLog;
use crate::hook::{HookConfig, LazyHookConfig};
use crate::platform::PlatformSpec;
use crate::project::{LazyProject, Project};
use crate::toolchain::{LazyToolchain, Toolchain};
use log::debug;

// 活动类型枚举，表示不同的 Volta 操作
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy)]
pub enum ActivityKind {
    Fetch,       // 获取
    Install,     // 安装
    Uninstall,   // 卸载
    List,        // 列表
    Current,     // 当前
    Default,     // 默认
    Pin,         // 固定
    Node,        // Node
    Npm,         // Npm
    Npx,         // Npx
    Pnpm,        // Pnpm
    Yarn,        // Yarn
    Volta,       // Volta
    Tool,        // 工具
    Help,        // 帮助
    Version,     // 版本
    Binary,      // 二进制
    Shim,        // 垫片
    Completions, // 补全
    Which,       // 查找
    Setup,       // 设置
    Run,         // 运行
    Args,        // 参数
}

// 为 ActivityKind 实现 Display trait
impl Display for ActivityKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        let s = match self {
            ActivityKind::Fetch => "fetch",
            ActivityKind::Install => "install",
            ActivityKind::Uninstall => "uninstall",
            ActivityKind::List => "list",
            ActivityKind::Current => "current",
            ActivityKind::Default => "default",
            ActivityKind::Pin => "pin",
            ActivityKind::Node => "node",
            ActivityKind::Npm => "npm",
            ActivityKind::Npx => "npx",
            ActivityKind::Pnpm => "pnpm",
            ActivityKind::Yarn => "yarn",
            ActivityKind::Volta => "volta",
            ActivityKind::Tool => "tool",
            ActivityKind::Help => "help",
            ActivityKind::Version => "version",
            ActivityKind::Binary => "binary",
            ActivityKind::Setup => "setup",
            ActivityKind::Shim => "shim",
            ActivityKind::Completions => "completions",
            ActivityKind::Which => "which",
            ActivityKind::Run => "run",
            ActivityKind::Args => "args",
        };
        f.write_str(s)
    }
}

/// 表示用户在执行 Volta 工具期间的状态。会话封装了工具调用环境的多个方面，包括：
///
/// - 当前目录
/// - 包含当前目录的 Node 项目树（如果有）
/// - Volta 钩子配置
/// - 本地获取的 Volta 工具库存
pub struct Session {
    hooks: LazyHookConfig,
    toolchain: LazyToolchain,
    project: LazyProject,
    event_log: EventLog,
}

impl Session {
    /// 构造一个新的 `Session`。
    pub fn init() -> Session {
        Session {
            hooks: LazyHookConfig::init(),
            toolchain: LazyToolchain::init(),
            project: LazyProject::init(),
            event_log: EventLog::init(),
        }
    }

    /// 获取当前 Node 项目的引用（如果有）。
    pub fn project(&self) -> Fallible<Option<&Project>> {
        self.project.get()
    }

    /// 获取当前 Node 项目的可变引用（如果有）。
    pub fn project_mut(&mut self) -> Fallible<Option<&mut Project>> {
        self.project.get_mut()
    }

    /// 返回用户的默认平台（如果有）。
    pub fn default_platform(&self) -> Fallible<Option<&PlatformSpec>> {
        self.toolchain.get().map(Toolchain::platform)
    }

    /// 返回当前项目的固定平台镜像（如果有）。
    pub fn project_platform(&self) -> Fallible<Option<&PlatformSpec>> {
        if let Some(project) = self.project()? {
            return Ok(project.platform());
        }
        Ok(None)
    }

    /// 获取当前工具链（默认平台规范）的引用。
    pub fn toolchain(&self) -> Fallible<&Toolchain> {
        self.toolchain.get()
    }

    /// 获取当前工具链的可变引用。
    pub fn toolchain_mut(&mut self) -> Fallible<&mut Toolchain> {
        self.toolchain.get_mut()
    }

    /// 获取钩子配置的引用。
    pub fn hooks(&self) -> Fallible<&HookConfig> {
        self.hooks.get(self.project()?)
    }

    // 以下方法用于添加不同类型的事件到事件日志

    pub fn add_event_start(&mut self, activity_kind: ActivityKind) {
        self.event_log.add_event_start(activity_kind)
    }
    pub fn add_event_end(&mut self, activity_kind: ActivityKind, exit_code: ExitCode) {
        self.event_log.add_event_end(activity_kind, exit_code)
    }
    pub fn add_event_tool_end(&mut self, activity_kind: ActivityKind, exit_code: i32) {
        self.event_log.add_event_tool_end(activity_kind, exit_code)
    }
    pub fn add_event_error(&mut self, activity_kind: ActivityKind, error: &VoltaError) {
        self.event_log.add_event_error(activity_kind, error)
    }

    // 发布事件日志
    fn publish_to_event_log(self) {
        let Self {
            project,
            hooks,
            mut event_log,
            ..
        } = self;
        let plugin_res = project
            .get()
            .and_then(|p| hooks.get(p))
            .map(|hooks| hooks.events().and_then(|e| e.publish.as_ref()));
        match plugin_res {
            Ok(plugin) => {
                event_log.add_event_args();
                event_log.publish(plugin);
            }
            Err(e) => {
                debug!("无法发布事件日志。\n{}", e);
            }
        }
    }

    // 退出程序并发布事件日志
    pub fn exit(self, code: ExitCode) -> ! {
        self.publish_to_event_log();
        code.exit();
    }

    // 退出工具并发布事件日志
    pub fn exit_tool(self, code: i32) -> ! {
        self.publish_to_event_log();
        exit(code);
    }
}

#[cfg(test)]
pub mod tests {

    use crate::session::Session;
    use std::env;
    use std::path::PathBuf;

    // 获取测试固件路径
    fn fixture_path(fixture_dir: &str) -> PathBuf {
        let mut cargo_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        cargo_manifest_dir.push("fixtures");
        cargo_manifest_dir.push(fixture_dir);
        cargo_manifest_dir
    }

    #[test]
    fn test_in_pinned_project() {
        // 测试固定项目
        let project_pinned = fixture_path("basic");
        env::set_current_dir(project_pinned).expect("无法设置当前目录");
        let pinned_session = Session::init();
        let pinned_platform = pinned_session.project_platform().expect("无法创建 Project");
        assert!(pinned_platform.is_some());

        // 测试未固定项目
        let project_unpinned = fixture_path("no_toolchain");
        env::set_current_dir(project_unpinned).expect("无法设置当前目录");
        let unpinned_session = Session::init();
        let unpinned_platform = unpinned_session
            .project_platform()
            .expect("无法创建 Project");
        assert!(unpinned_platform.is_none());
    }
}
