use std::fs::write;

use crate::error::{Context, ErrorKind, Fallible};
use crate::fs::touch;
use crate::layout::volta_home;
use crate::platform::PlatformSpec;
use log::debug;
use node_semver::Version;
use once_cell::unsync::OnceCell;
use readext::ReadExt;

pub mod serial;

/// 懒加载的工具链
/// Lazily loaded toolchain
pub struct LazyToolchain {
    toolchain: OnceCell<Toolchain>,
}

impl LazyToolchain {
    /// 创建一个新的 `LazyToolchain`
    /// Creates a new `LazyToolchain`
    pub fn init() -> Self {
        LazyToolchain {
            toolchain: OnceCell::new(),
        }
    }

    /// 强制加载工具链并返回其不可变引用
    /// Forces loading of the toolchain and returns an immutable reference to it
    pub fn get(&self) -> Fallible<&Toolchain> {
        self.toolchain.get_or_try_init(Toolchain::current)
    }

    /// 强制加载工具链并返回其可变引用
    /// Forces loading of the toolchain and returns a mutable reference to it
    pub fn get_mut(&mut self) -> Fallible<&mut Toolchain> {
        let _ = self.toolchain.get_or_try_init(Toolchain::current)?;
        Ok(self.toolchain.get_mut().unwrap())
    }
}

/// 工具链结构体
/// Toolchain struct
pub struct Toolchain {
    platform: Option<PlatformSpec>,
}

impl Toolchain {
    /// 获取当前工具链
    /// Get the current toolchain
    fn current() -> Fallible<Toolchain> {
        let path = volta_home()?.default_platform_file();
        let src = touch(path)
            .and_then(|mut file| file.read_into_string())
            .with_context(|| ErrorKind::ReadPlatformError {
                file: path.to_owned(),
            })?;

        let platform: Option<PlatformSpec> = serial::Platform::try_from(src)?.into();
        if platform.is_some() {
            debug!("找到默认配置文件：'{}'", path.display());
        }
        Ok(Toolchain { platform })
    }

    /// 获取平台规格
    /// Get the platform specification
    pub fn platform(&self) -> Option<&PlatformSpec> {
        self.platform.as_ref()
    }

    /// 在默认平台文件中设置活动的 Node 版本
    /// Set the active Node version in the default platform file
    pub fn set_active_node(&mut self, node_version: &Version) -> Fallible<()> {
        let mut dirty = false;

        match self.platform.as_mut() {
            Some(platform) => {
                if platform.node != *node_version {
                    platform.node = node_version.clone();
                    dirty = true;
                }
            }
            None => {
                self.platform = Some(PlatformSpec {
                    node: node_version.clone(),
                    npm: None,
                    pnpm: None,
                    yarn: None,
                });
                dirty = true;
            }
        }

        if dirty {
            self.save()?;
        }

        Ok(())
    }

    /// 在默认平台文件中设置活动的 Yarn 版本
    /// Set the active Yarn version in the default platform file
    pub fn set_active_yarn(&mut self, yarn: Option<Version>) -> Fallible<()> {
        if let Some(platform) = self.platform.as_mut() {
            if platform.yarn != yarn {
                platform.yarn = yarn;
                self.save()?;
            }
        } else if yarn.is_some() {
            return Err(ErrorKind::NoDefaultNodeVersion {
                tool: "Yarn".into(),
            }
            .into());
        }

        Ok(())
    }

    /// 在默认平台文件中设置活动的 pnpm 版本
    /// Set the active pnpm version in the default platform file
    pub fn set_active_pnpm(&mut self, pnpm: Option<Version>) -> Fallible<()> {
        if let Some(platform) = self.platform.as_mut() {
            if platform.pnpm != pnpm {
                platform.pnpm = pnpm;
                self.save()?;
            }
        } else if pnpm.is_some() {
            return Err(ErrorKind::NoDefaultNodeVersion {
                tool: "pnpm".into(),
            }
            .into());
        }

        Ok(())
    }

    /// 在默认平台文件中设置活动的 Npm 版本
    /// Set the active Npm version in the default platform file
    pub fn set_active_npm(&mut self, npm: Option<Version>) -> Fallible<()> {
        if let Some(platform) = self.platform.as_mut() {
            if platform.npm != npm {
                platform.npm = npm;
                self.save()?;
            }
        } else if npm.is_some() {
            return Err(ErrorKind::NoDefaultNodeVersion { tool: "npm".into() }.into());
        }

        Ok(())
    }

    /// 保存工具链配置
    /// Save the toolchain configuration
    pub fn save(&self) -> Fallible<()> {
        let path = volta_home()?.default_platform_file();
        let result = match &self.platform {
            Some(platform) => {
                let src = serial::Platform::of(platform).into_json()?;
                write(path, src)
            }
            None => write(path, "{}"),
        };
        result.with_context(|| ErrorKind::WritePlatformError {
            file: path.to_owned(),
        })
    }
}
