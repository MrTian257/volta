//! 提供 Node 发行版的获取器

use std::env;
use std::fs::{read_to_string, write, File};
use std::path::{Path, PathBuf};

use super::NodeVersion;
use crate::error::{Context, ErrorKind, Fallible};
use crate::fs::{create_staging_dir, create_staging_file, rename};
use crate::hook::ToolHooks;
use crate::layout::volta_home;
use crate::style::{progress_bar, tool_version};
use crate::tool::{self, download_tool_error, Node};
use crate::version::{parse_version, VersionSpec};
use archive::{self, Archive};
use cfg_if::cfg_if;
use fs_utils::ensure_containing_dir_exists;
use log::{debug, info};
use node_semver::Version;
use serde::Deserialize;

cfg_if! {
    if #[cfg(feature = "mock-network")] {
        // TODO: 我们需要重新考虑我们的模拟策略，因为 mockito 已弃用 SERVER_URL 常量：
        // 由于我们的验收测试在单独的进程中运行二进制文件，
        // 我们不能使用 `mockito::server_url()`，它依赖于共享内存。
        fn public_node_server_root() -> String {
            #[allow(deprecated)]
            mockito::SERVER_URL.to_string()
        }
    } else {
        // NODE_MIRROR=https://mirrors.aliyun.com/nodejs-release
        fn public_node_server_root() -> String {
            match env::var_os("ENV_NODE_MIRROR") {
                Some(val) => format!("{}", val.to_string_lossy()),
                None => "https://mirrors.aliyun.com/nodejs-release".to_string()
            }
        }
    }
}

// 返回 npm 清单文件的路径
fn npm_manifest_path(version: &Version) -> PathBuf {
    let mut manifest = PathBuf::from(Node::archive_basename(version));

    #[cfg(unix)]
    manifest.push("lib");

    manifest.push("node_modules");
    manifest.push("npm");
    manifest.push("package.json");

    manifest
}

// 获取指定版本的 Node
pub fn fetch(version: &Version, hooks: Option<&ToolHooks<Node>>) -> Fallible<NodeVersion> {
    let home = volta_home()?;
    let node_dir = home.node_inventory_dir();
    let cache_file = node_dir.join(Node::archive_filename(version));

    let (archive, staging) = match load_cached_distro(&cache_file) {
        Some(archive) => {
            info!(
                "从缓存的归档文件 '{}' 加载 {}",
                cache_file.display(),
                tool_version("node", version)
            );
            (archive, None)
        }
        None => {
            let staging = create_staging_file()?;
            let remote_url = determine_remote_url(version, hooks)?;
            let archive = fetch_remote_distro(version, &remote_url, staging.path())?;
            (archive, Some(staging))
        }
    };

    let node_version = unpack_archive(archive, version)?;

    if let Some(staging_file) = staging {
        ensure_containing_dir_exists(&cache_file).with_context(|| {
            ErrorKind::ContainingDirError {
                path: cache_file.clone(),
            }
        })?;
        staging_file
            .persist(cache_file)
            .with_context(|| ErrorKind::PersistInventoryError {
                tool: "Node".into(),
            })?;
    }

    Ok(node_version)
}

// 将 Node 归档解压到镜像目录，使其可以使用
fn unpack_archive(archive: Box<dyn Archive>, version: &Version) -> Fallible<NodeVersion> {
    let temp = create_staging_dir()?;
    debug!("将 Node 解压到 '{}'", temp.path().display());

    let progress = progress_bar(
        archive.origin(),
        &tool_version("node", version),
        archive.compressed_size(),
    );
    let version_string = version.to_string();

    archive
        .unpack(temp.path(), &mut |_, read| {
            progress.inc(read as u64);
        })
        .with_context(|| ErrorKind::UnpackArchiveError {
            tool: "Node".into(),
            version: version_string.clone(),
        })?;

    // 将 npm 版本号保存到该发行版的 npm 版本文件中
    let npm_package_json = temp.path().join(npm_manifest_path(version));
    let npm = Manifest::version(&npm_package_json)?;
    save_default_npm_version(version, &npm)?;

    let dest = volta_home()?.node_image_dir(&version_string);
    ensure_containing_dir_exists(&dest)
        .with_context(|| ErrorKind::ContainingDirError { path: dest.clone() })?;

    rename(temp.path().join(Node::archive_basename(version)), &dest).with_context(|| {
        ErrorKind::SetupToolImageError {
            tool: "Node".into(),
            version: version_string,
            dir: dest.clone(),
        }
    })?;

    progress.finish_and_clear();

    // 注意：我们在进度条完成后写入这些，以避免重新渲染进度时出现显示错误
    debug!("保存捆绑的 npm 版本 ({})", npm);
    debug!("在 '{}' 中安装 Node", dest.display());

    Ok(NodeVersion {
        runtime: version.clone(),
        npm,
    })
}

// 如果归档文件有效，则返回它。它可能在下载过程中被损坏或中断。
// ISSUE(#134) - 验证校验和
fn load_cached_distro(file: &Path) -> Option<Box<dyn Archive>> {
    if file.is_file() {
        let file = File::open(file).ok()?;
        archive::load_native(file).ok()
    } else {
        None
    }
}

// 确定要下载的远程 URL，如果可用，则使用钩子
fn determine_remote_url(version: &Version, hooks: Option<&ToolHooks<Node>>) -> Fallible<String> {
    let distro_file_name = Node::archive_filename(version);
    match hooks {
        Some(&ToolHooks {
            distro: Some(ref hook),
            ..
        }) => {
            debug!("使用 node.distro 钩子确定下载 URL");
            hook.resolve(version, &distro_file_name)
        }
        _ => Ok(format!(
            "{}/v{}/{}",
            public_node_server_root(),
            version,
            distro_file_name
        )),
    }
}

// 从互联网获取发行版归档
fn fetch_remote_distro(
    version: &Version,
    url: &str,
    staging_path: &Path,
) -> Fallible<Box<dyn Archive>> {
    info!("从 {} 下载 {}", url, tool_version("node", version));
    archive::fetch_native(url, staging_path).with_context(download_tool_error(
        tool::Spec::Node(VersionSpec::Exact(version.clone())),
        url,
    ))
}

// npm 的 `package.json` 文件中我们关心的部分
#[derive(Deserialize)]
struct Manifest {
    version: String,
}

impl Manifest {
    // 从 package.json 文件中解析版本
    fn version(path: &Path) -> Fallible<Version> {
        let file = File::open(path).with_context(|| ErrorKind::ReadNpmManifestError)?;
        let manifest: Manifest =
            serde_json::de::from_reader(file).with_context(|| ErrorKind::ParseNpmManifestError)?;
        parse_version(manifest.version)
    }
}

// 加载本地 npm 版本文件以确定给定 Node 版本的默认 npm 版本
pub fn load_default_npm_version(node: &Version) -> Fallible<Version> {
    let npm_version_file_path = volta_home()?.node_npm_version_file(&node.to_string());
    let npm_version =
        read_to_string(&npm_version_file_path).with_context(|| ErrorKind::ReadDefaultNpmError {
            file: npm_version_file_path,
        })?;
    parse_version(npm_version)
}

// 为给定的 Node 版本将默认 npm 版本保存到文件系统
fn save_default_npm_version(node: &Version, npm: &Version) -> Fallible<()> {
    let npm_version_file_path = volta_home()?.node_npm_version_file(&node.to_string());
    write(&npm_version_file_path, npm.to_string().as_bytes()).with_context(|| {
        ErrorKind::WriteDefaultNpmError {
            file: npm_version_file_path,
        }
    })
}
