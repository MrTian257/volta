//! 提供修改第三方可执行文件垫片的实用工具

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use crate::error::{Context, ErrorKind, Fallible, VoltaError};
use crate::fs::read_dir_eager;
use crate::layout::volta_home;
use crate::sync::VoltaLock;
use log::debug;

pub use platform::create;

// 为指定目录重新生成垫片
pub fn regenerate_shims_for_dir(dir: &Path) -> Fallible<()> {
    // 如果可能，获取Volta目录的锁，以防止并发更改
    let _lock = VoltaLock::acquire();
    debug!("正在为目录重建垫片: {}", dir.display());
    for shim_name in get_shim_list_deduped(dir)?.iter() {
        delete(shim_name)?;
        create(shim_name)?;
    }

    Ok(())
}

// 获取去重后的垫片列表
fn get_shim_list_deduped(dir: &Path) -> Fallible<HashSet<String>> {
    let contents = read_dir_eager(dir).with_context(|| ErrorKind::ReadDirError {
        dir: dir.to_owned(),
    })?;

    #[cfg(unix)]
    {
        let mut shims: HashSet<String> =
            contents.filter_map(platform::entry_to_shim_name).collect();
        // 添加默认的垫片
        shims.insert("node".into());
        shims.insert("npm".into());
        shims.insert("npx".into());
        shims.insert("pnpm".into());
        shims.insert("yarn".into());
        shims.insert("yarnpkg".into());
        Ok(shims)
    }

    #[cfg(windows)]
    {
        // 在Windows上，默认垫片安装在Program Files中，所以我们不需要在这里生成它们
        Ok(contents.filter_map(platform::entry_to_shim_name).collect())
    }
}

// 垫片操作的结果枚举
#[derive(PartialEq, Eq)]
pub enum ShimResult {
    Created,       // 创建成功
    AlreadyExists, // 已经存在
    Deleted,       // 删除成功
    DoesntExist,   // 不存在
}

// 删除指定的垫片
pub fn delete(shim_name: &str) -> Fallible<ShimResult> {
    let shim = volta_home()?.shim_file(shim_name);

    #[cfg(windows)]
    platform::delete_git_bash_script(shim_name)?;

    match fs::remove_file(shim) {
        Ok(_) => Ok(ShimResult::Deleted),
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                Ok(ShimResult::DoesntExist)
            } else {
                Err(VoltaError::from_source(
                    err,
                    ErrorKind::ShimRemoveError {
                        name: shim_name.to_string(),
                    },
                ))
            }
        }
    }
}

#[cfg(unix)]
mod platform {
    //! Unix特定的垫片工具
    //!
    //! 在macOS和Linux上，创建垫片涉及创建到`volta-shim`可执行文件的符号链接。
    //! 此外，从目录条目中过滤垫片意味着查找符号链接并忽略实际的二进制文件。
    use std::ffi::OsStr;
    use std::fs::{DirEntry, Metadata};
    use std::io;

    use super::ShimResult;
    use crate::error::{ErrorKind, Fallible, VoltaError};
    use crate::fs::symlink_file;
    use crate::layout::{volta_home, volta_install};

    // 创建垫片
    pub fn create(shim_name: &str) -> Fallible<ShimResult> {
        let executable = volta_install()?.shim_executable();
        let shim = volta_home()?.shim_file(shim_name);

        match symlink_file(executable, shim) {
            Ok(_) => Ok(ShimResult::Created),
            Err(err) => {
                if err.kind() == io::ErrorKind::AlreadyExists {
                    Ok(ShimResult::AlreadyExists)
                } else {
                    Err(VoltaError::from_source(
                        err,
                        ErrorKind::ShimCreateError {
                            name: shim_name.to_string(),
                        },
                    ))
                }
            }
        }
    }

    // 从目录条目获取垫片名称
    pub fn entry_to_shim_name((entry, metadata): (DirEntry, Metadata)) -> Option<String> {
        if metadata.file_type().is_symlink() {
            entry
                .path()
                .file_stem()
                .and_then(OsStr::to_str)
                .map(ToOwned::to_owned)
        } else {
            None
        }
    }
}

#[cfg(windows)]
mod platform {
    //! Windows特定的垫片工具
    //!
    //! 在Windows上，创建垫片涉及创建一个小的.cmd脚本，而不是符号链接。
    //! 这允许我们创建垫片而无需管理员权限或开发者模式。此外，为了支持Git Bash，
    //! 我们创建一个类似的具有bash语法的脚本，该脚本没有文件扩展名。
    //! 这允许Powershell和Cmd忽略它，而Bash将其检测为可执行脚本。
    //!
    //! 最后，过滤目录条目以查找垫片文件涉及查找.cmd文件。
    use std::ffi::OsStr;
    use std::fs::{write, DirEntry, Metadata};

    use super::ShimResult;
    use crate::error::{Context, ErrorKind, Fallible};
    use crate::fs::remove_file_if_exists;
    use crate::layout::volta_home;

    // CMD脚本内容
    const SHIM_SCRIPT_CONTENTS: &str = r#"@echo off
volta run %~n0 %*
"#;

    // Git Bash脚本内容
    const GIT_BASH_SCRIPT_CONTENTS: &str = r#"#!/bin/bash
volta run "$(basename $0)" "$@""#;

    // 创建垫片
    pub fn create(shim_name: &str) -> Fallible<ShimResult> {
        let shim = volta_home()?.shim_file(shim_name);

        write(shim, SHIM_SCRIPT_CONTENTS).with_context(|| ErrorKind::ShimCreateError {
            name: shim_name.to_owned(),
        })?;

        let git_bash_script = volta_home()?.shim_git_bash_script_file(shim_name);

        write(git_bash_script, GIT_BASH_SCRIPT_CONTENTS).with_context(|| {
            ErrorKind::ShimCreateError {
                name: shim_name.to_owned(),
            }
        })?;

        Ok(ShimResult::Created)
    }

    // 从目录条目获取垫片名称
    pub fn entry_to_shim_name((entry, _): (DirEntry, Metadata)) -> Option<String> {
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "cmd") {
            path.file_stem()
                .and_then(OsStr::to_str)
                .map(ToOwned::to_owned)
        } else {
            None
        }
    }

    // 删除Git Bash脚本
    pub fn delete_git_bash_script(shim_name: &str) -> Fallible<()> {
        let script_path = volta_home()?.shim_git_bash_script_file(shim_name);
        remove_file_if_exists(script_path).with_context(|| ErrorKind::ShimRemoveError {
            name: shim_name.to_string(),
        })
    }
}
