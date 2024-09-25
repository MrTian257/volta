//! 提供使用 NodeJS 索引将 Node 需求解析为特定版本的功能

use std::env;
use std::fs::File;
use std::io::Write;
use std::time::{Duration, SystemTime};

use super::super::registry_fetch_error;
use super::metadata::{NodeEntry, NodeIndex, RawNodeIndex};
use crate::error::{Context, ErrorKind, Fallible};
use crate::fs::{create_staging_file, read_file};
use crate::hook::ToolHooks;
use crate::layout::volta_home;
use crate::session::Session;
use crate::style::progress_spinner;
use crate::tool::Node;
use crate::version::{VersionSpec, VersionTag};
use attohttpc::header::HeaderMap;
use attohttpc::Response;
use cfg_if::cfg_if;
use fs_utils::ensure_containing_dir_exists;
use headers::{CacheControl, Expires, HeaderMapExt};
use log::debug;
use node_semver::{Range, Version};

// 问题 (#86): 将公共仓库 URL 移至配置文件
cfg_if! {
    if #[cfg(feature = "mock-network")] {
        // TODO: 由于 mockito 弃用了 SERVER_URL 常量，我们需要重新考虑我们的模拟策略：
        // 因为我们的验收测试在单独的进程中运行二进制文件，
        // 我们不能使用 `mockito::server_url()`，它依赖于共享内存。
        #[allow(deprecated)]
        const SERVER_URL: &str = mockito::SERVER_URL;
        fn public_node_version_index() -> String {
            format!("{}/node-dist/index.json", SERVER_URL)
        }
    } else {
        // NODE_MIRROR=https://mirrors.aliyun.com/nodejs-release
        /// 返回公共 Node 服务器上可用 Node 版本索引的 URL。
        fn public_node_version_index() -> String {
            // "https://mirrors.aliyun.com/nodejs-release/index.json".to_string()
            match env::var_os("ENV_NODE_MIRROR") {
                Some(val) =>  format!("{}/index.json", val.to_string_lossy()),
                None => "https://mirrors.aliyun.com/nodejs-release/index.json".to_string()
            }
        }
    }
}

/// 解析 Node 版本
pub fn resolve(matching: VersionSpec, session: &mut Session) -> Fallible<Version> {
    let hooks = session.hooks()?.node();
    match matching {
        VersionSpec::Semver(requirement) => resolve_semver(requirement, hooks),
        VersionSpec::Exact(version) => Ok(version),
        VersionSpec::None | VersionSpec::Tag(VersionTag::Lts) => resolve_lts(hooks),
        VersionSpec::Tag(VersionTag::Latest) => resolve_latest(hooks),
        // Node 没有"标记"版本（除了 'latest' 和 'lts'），所以自定义标记总是会出错
        VersionSpec::Tag(VersionTag::Custom(tag)) => {
            Err(ErrorKind::NodeVersionNotFound { matching: tag }.into())
        }
    }
}

/// 解析最新的 Node 版本
fn resolve_latest(hooks: Option<&ToolHooks<Node>>) -> Fallible<Version> {
    // 注意：这假设注册表总是按从最新到最旧的顺序生成列表。
    // 当我们记录插件 API 时，这应该被指定为一个要求。
    let url = match hooks {
        Some(&ToolHooks {
            latest: Some(ref hook),
            ..
        }) => {
            debug!("使用 node.latest 钩子确定 node 索引 URL");
            hook.resolve("index.json")?
        }
        _ => public_node_version_index(),
    };
    let version_opt = match_node_version(&url, |_| true)?;

    match version_opt {
        Some(version) => {
            debug!("从 {} 找到最新的 node 版本 ({})", url, version);
            Ok(version)
        }
        None => Err(ErrorKind::NodeVersionNotFound {
            matching: "latest".into(),
        }
        .into()),
    }
}

/// 解析最新的 LTS Node 版本
fn resolve_lts(hooks: Option<&ToolHooks<Node>>) -> Fallible<Version> {
    let url = match hooks {
        Some(&ToolHooks {
            index: Some(ref hook),
            ..
        }) => {
            debug!("使用 node.index 钩子确定 node 索引 URL");
            hook.resolve("index.json")?
        }
        _ => public_node_version_index(),
    };
    let version_opt = match_node_version(&url, |&NodeEntry { lts, .. }| lts)?;

    match version_opt {
        Some(version) => {
            debug!("从 {} 找到最新的 LTS node 版本 ({})", url, version);
            Ok(version)
        }
        None => Err(ErrorKind::NodeVersionNotFound {
            matching: "lts".into(),
        }
        .into()),
    }
}

/// 解析符合语义化版本要求的 Node 版本
fn resolve_semver(matching: Range, hooks: Option<&ToolHooks<Node>>) -> Fallible<Version> {
    let url = match hooks {
        Some(&ToolHooks {
            index: Some(ref hook),
            ..
        }) => {
            debug!("使用 node.index 钩子确定 node 索引 URL");
            hook.resolve("index.json")?
        }
        _ => public_node_version_index(),
    };
    let version_opt = match_node_version(&url, |NodeEntry { version, .. }| {
        matching.satisfies(version)
    })?;

    match version_opt {
        Some(version) => {
            debug!("从 {} 找到 node@{} 匹配要求 '{}'", url, version, matching);
            Ok(version)
        }
        None => Err(ErrorKind::NodeVersionNotFound {
            matching: matching.to_string(),
        }
        .into()),
    }
}

/// 匹配符合条件的 Node 版本
fn match_node_version(
    url: &str,
    predicate: impl Fn(&NodeEntry) -> bool,
) -> Fallible<Option<Version>> {
    let index: NodeIndex = resolve_node_versions(url)?.into();
    let mut entries = index.entries.into_iter();
    Ok(entries
        .find(predicate)
        .map(|NodeEntry { version, .. }| version))
}

/// 如果存在且未过期，从 Node 缓存中读取公共索引
fn read_cached_opt(url: &str) -> Fallible<Option<RawNodeIndex>> {
    let expiry_file = volta_home()?.node_index_expiry_file();
    let expiry = read_file(expiry_file).with_context(|| ErrorKind::ReadNodeIndexExpiryError {
        file: expiry_file.to_owned(),
    })?;

    if !expiry
        .map(|date| httpdate::parse_http_date(&date))
        .transpose()
        .with_context(|| ErrorKind::ParseNodeIndexExpiryError)?
        .is_some_and(|expiry_date| SystemTime::now() < expiry_date)
    {
        return Ok(None);
    };

    let index_file = volta_home()?.node_index_file();
    let cached = read_file(index_file).with_context(|| ErrorKind::ReadNodeIndexCacheError {
        file: index_file.to_owned(),
    })?;

    let Some(json) = cached
        .as_ref()
        .and_then(|content| content.strip_prefix(url))
    else {
        return Ok(None);
    };

    serde_json::de::from_str(json).with_context(|| ErrorKind::ParseNodeIndexCacheError)
}

/// 获取 HTTP 响应的缓存最大年龄
fn max_age(headers: &HeaderMap) -> Duration {
    const FOUR_HOURS: Duration = Duration::from_secs(4 * 60 * 60);
    headers
        .typed_get::<CacheControl>()
        .and_then(|cache_control| cache_control.max_age())
        .unwrap_or(FOUR_HOURS)
}

/// 解析 Node 版本
fn resolve_node_versions(url: &str) -> Fallible<RawNodeIndex> {
    match read_cached_opt(url)? {
        Some(serial) => {
            debug!("找到有效的 Node 版本索引缓存");
            Ok(serial)
        }
        None => {
            debug!("未找到 Node 索引缓存或缓存无效");
            let spinner = progress_spinner(format!("获取公共注册表: {}", url));

            let (_, headers, response) = attohttpc::get(url)
                .send()
                .and_then(Response::error_for_status)
                .with_context(registry_fetch_error("Node", url))?
                .split();

            let expires = headers
                .typed_get::<Expires>()
                .map(SystemTime::from)
                .unwrap_or_else(|| SystemTime::now() + max_age(&headers));

            let response_text = response
                .text()
                .with_context(registry_fetch_error("Node", url))?;

            let index: RawNodeIndex =
                serde_json::de::from_str(&response_text).with_context(|| {
                    ErrorKind::ParseNodeIndexError {
                        from_url: url.to_string(),
                    }
                })?;

            let cached = create_staging_file()?;

            let mut cached_file: &File = cached.as_file();
            writeln!(cached_file, "{}", url)
                .and_then(|_| cached_file.write(response_text.as_bytes()))
                .with_context(|| ErrorKind::WriteNodeIndexCacheError {
                    file: cached.path().to_path_buf(),
                })?;

            let index_cache_file = volta_home()?.node_index_file();
            ensure_containing_dir_exists(&index_cache_file).with_context(|| {
                ErrorKind::ContainingDirError {
                    path: index_cache_file.to_owned(),
                }
            })?;
            cached.persist(index_cache_file).with_context(|| {
                ErrorKind::WriteNodeIndexCacheError {
                    file: index_cache_file.to_owned(),
                }
            })?;

            let expiry = create_staging_file()?;
            let mut expiry_file: &File = expiry.as_file();

            write!(expiry_file, "{}", httpdate::fmt_http_date(expires)).with_context(|| {
                ErrorKind::WriteNodeIndexExpiryError {
                    file: expiry.path().to_path_buf(),
                }
            })?;

            let index_expiry_file = volta_home()?.node_index_expiry_file();
            ensure_containing_dir_exists(&index_expiry_file).with_context(|| {
                ErrorKind::ContainingDirError {
                    path: index_expiry_file.to_owned(),
                }
            })?;
            expiry.persist(index_expiry_file).with_context(|| {
                ErrorKind::WriteNodeIndexExpiryError {
                    file: index_expiry_file.to_owned(),
                }
            })?;

            spinner.finish_and_clear();
            Ok(index)
        }
    }
}
