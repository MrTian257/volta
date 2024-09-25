use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use super::executor::{Executor, ToolCommand, ToolKind};
use super::{debug_active_image, debug_no_platform};
use crate::error::{Context, ErrorKind, Fallible};
use crate::layout::volta_home;
use crate::platform::{Platform, Sourced, System};
use crate::session::Session;
use crate::tool::package::BinConfig;
use log::debug;

/// 确定第三方二进制文件的正确运行命令
///
/// 将检测是否应该委托给项目本地版本或使用默认版本
pub(super) fn command(exe: &OsStr, args: &[OsString], session: &mut Session) -> Fallible<Executor> {
    let bin = exe.to_string_lossy().to_string();
    // 首先尝试使用项目工具链
    if let Some(project) = session.project()? {
        // 检查可执行文件是否为直接依赖
        if project.has_direct_bin(exe)? {
            match project.find_bin(exe) {
                Some(path_to_bin) => {
                    debug!("在项目中找到 {} 位于 '{}'", bin, path_to_bin.display());

                    let platform = Platform::current(session)?;
                    return Ok(ToolCommand::new(
                        path_to_bin,
                        args,
                        platform,
                        ToolKind::ProjectLocalBinary(bin),
                    )
                    .into());
                }
                None => {
                    if project.needs_yarn_run() {
                        debug!("项目需要使用 yarn 运行命令，使用 'yarn' 调用 {}", bin);
                        let platform = Platform::current(session)?;
                        let mut exe_and_args = vec![exe.to_os_string()];
                        exe_and_args.extend_from_slice(args);
                        return Ok(ToolCommand::new(
                            "yarn",
                            exe_and_args,
                            platform,
                            ToolKind::Yarn,
                        )
                        .into());
                    } else {
                        return Err(ErrorKind::ProjectLocalBinaryNotFound {
                            command: exe.to_string_lossy().to_string(),
                        }
                        .into());
                    }
                }
            }
        }
    }

    // 尝试使用默认工具链
    if let Some(default_tool) = DefaultBinary::from_name(exe, session)? {
        debug!(
            "在 '{}' 中找到默认的 {}",
            bin,
            default_tool.bin_path.display()
        );

        let mut command = ToolCommand::new(
            default_tool.bin_path,
            args,
            Some(default_tool.platform),
            ToolKind::DefaultBinary(bin),
        );
        command.env("NODE_PATH", shared_module_path()?);

        return Ok(command.into());
    }

    // 此时，二进制文件对 Volta 未知，因此我们没有平台来执行它
    // 这种情况应该很少见，因为任何我们有 shim 的东西都应该有一个配置文件来加载
    Ok(ToolCommand::new(exe, args, None, ToolKind::DefaultBinary(bin)).into())
}

/// 确定项目本地二进制文件的执行上下文（PATH 和失败错误消息）
pub(super) fn local_execution_context(
    tool: String,
    platform: Option<Platform>,
    session: &mut Session,
) -> Fallible<(OsString, ErrorKind)> {
    match platform {
        Some(plat) => {
            let image = plat.checkout(session)?;
            let path = image.path()?;
            debug_active_image(&image);

            Ok((
                path,
                ErrorKind::ProjectLocalBinaryExecError { command: tool },
            ))
        }
        None => {
            let path = System::path()?;
            debug_no_platform();

            Ok((path, ErrorKind::NoPlatform))
        }
    }
}

/// 确定默认二进制文件的执行上下文（PATH 和失败错误消息）
pub(super) fn default_execution_context(
    tool: String,
    platform: Option<Platform>,
    session: &mut Session,
) -> Fallible<(OsString, ErrorKind)> {
    match platform {
        Some(plat) => {
            let image = plat.checkout(session)?;
            let path = image.path()?;
            debug_active_image(&image);

            Ok((path, ErrorKind::BinaryExecError))
        }
        None => {
            let path = System::path()?;
            debug_no_platform();

            Ok((path, ErrorKind::BinaryNotFound { name: tool }))
        }
    }
}

/// 默认二进制文件的位置和执行上下文信息
///
/// 从 Volta 目录中的配置文件获取，表示当用户在没有给定 bin 作为依赖项的项目之外执行时执行的二进制文件。
pub struct DefaultBinary {
    pub bin_path: PathBuf,
    pub platform: Platform,
}

impl DefaultBinary {
    pub fn from_config(bin_config: BinConfig, session: &mut Session) -> Fallible<Self> {
        let package_dir = volta_home()?.package_image_dir(&bin_config.package);
        let mut bin_path = bin_config.manager.binary_dir(package_dir);
        bin_path.push(&bin_config.name);

        // 如果用户没有在此二进制文件的平台中设置 yarn，则使用默认值
        // 这是必要的，因为某些工具（例如带有 `--yarn` 选项的 ember-cli）会调用 `yarn`
        let yarn = match bin_config.platform.yarn {
            Some(yarn) => Some(yarn),
            None => session
                .default_platform()?
                .and_then(|plat| plat.yarn.clone()),
        };
        let platform = Platform {
            node: Sourced::with_binary(bin_config.platform.node),
            npm: bin_config.platform.npm.map(Sourced::with_binary),
            pnpm: bin_config.platform.pnpm.map(Sourced::with_binary),
            yarn: yarn.map(Sourced::with_binary),
        };

        Ok(DefaultBinary { bin_path, platform })
    }

    /// 通过名称加载默认二进制文件的信息（如果可用）
    ///
    /// 这里的 `None` 响应意味着找不到工具信息。要么工具名称不是有效的 UTF-8 字符串，要么工具配置不存在。
    pub fn from_name(tool_name: &OsStr, session: &mut Session) -> Fallible<Option<Self>> {
        let bin_config_file = match tool_name.to_str() {
            Some(name) => volta_home()?.default_tool_bin_config(name),
            None => return Ok(None),
        };

        match BinConfig::from_file_if_exists(bin_config_file)? {
            Some(config) => DefaultBinary::from_config(config, session).map(Some),
            None => Ok(None),
        }
    }
}

/// 确定 NODE_PATH 的值，在前面添加共享库目录
///
/// 这将确保全局 bin 可以 `require` 其他全局库
fn shared_module_path() -> Fallible<OsString> {
    let node_path = match env::var("NODE_PATH") {
        Ok(path) => envoy::Var::from(path),
        Err(_) => envoy::Var::from(""),
    };

    node_path
        .split()
        .prefix_entry(volta_home()?.shared_lib_root())
        .join()
        .with_context(|| ErrorKind::BuildPathError)
}
