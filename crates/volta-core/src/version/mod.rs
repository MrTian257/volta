use std::fmt;
use std::str::FromStr;

use crate::error::{Context, ErrorKind, Fallible, VoltaError};
use node_semver::{Range, Version};

mod serial;

// 版本规格枚举，用于表示不同类型的版本信息
#[derive(Debug, Default)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum VersionSpec {
    /// 未指定版本（默认）
    #[default]
    None,

    /// 语义化版本范围
    Semver(Range),

    /// 精确版本
    Exact(Version),

    /// 任意版本标签
    Tag(VersionTag),
}

// 版本标签枚举，用于表示特殊的版本标签
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum VersionTag {
    /// 'latest' 标签，所有包都存在的特殊情况
    Latest,

    /// 'lts' 标签，Node 的特殊情况
    Lts,

    /// 自定义标签版本
    Custom(String),
}

// 为 VersionSpec 实现 Display trait
impl fmt::Display for VersionSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionSpec::None => write!(f, "<default>"),
            VersionSpec::Semver(req) => req.fmt(f),
            VersionSpec::Exact(version) => version.fmt(f),
            VersionSpec::Tag(tag) => tag.fmt(f),
        }
    }
}

// 为 VersionTag 实现 Display trait
impl fmt::Display for VersionTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionTag::Latest => write!(f, "latest"),
            VersionTag::Lts => write!(f, "lts"),
            VersionTag::Custom(s) => s.fmt(f),
        }
    }
}

// 为 VersionSpec 实现 FromStr trait
impl FromStr for VersionSpec {
    type Err = VoltaError;

    fn from_str(s: &str) -> Fallible<Self> {
        if let Ok(version) = parse_version(s) {
            Ok(VersionSpec::Exact(version))
        } else if let Ok(req) = parse_requirements(s) {
            Ok(VersionSpec::Semver(req))
        } else {
            s.parse().map(VersionSpec::Tag)
        }
    }
}

// 为 VersionTag 实现 FromStr trait
impl FromStr for VersionTag {
    type Err = VoltaError;

    fn from_str(s: &str) -> Fallible<Self> {
        if s == "latest" {
            Ok(VersionTag::Latest)
        } else if s == "lts" {
            Ok(VersionTag::Lts)
        } else {
            Ok(VersionTag::Custom(s.into()))
        }
    }
}

// 解析版本要求
pub fn parse_requirements(s: impl AsRef<str>) -> Fallible<Range> {
    let s = s.as_ref();
    serial::parse_requirements(s)
        .with_context(|| ErrorKind::VersionParseError { version: s.into() })
}

// 解析版本
pub fn parse_version(s: impl AsRef<str>) -> Fallible<Version> {
    let s = s.as_ref();
    s.parse()
        .with_context(|| ErrorKind::VersionParseError { version: s.into() })
}

// 如果存在，移除版本字符串开头的 'v'
fn trim_version(s: &str) -> &str {
    let s = s.trim();
    match s.strip_prefix('v') {
        Some(stripped) => stripped,
        None => s,
    }
}

// Version 的自定义序列化和反序列化
// 因为 Version 不能直接与 serde 一起使用
pub mod version_serde {
    use node_semver::Version;
    use serde::de::{Error, Visitor};
    use serde::{self, Deserializer, Serializer};
    use std::fmt;

    struct VersionVisitor;

    impl<'de> Visitor<'de> for VersionVisitor {
        type Value = Version;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("string")
        }

        // 从字符串解析版本
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Version::parse(super::trim_version(value)).map_err(Error::custom)
        }
    }

    pub fn serialize<S>(version: &Version, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(&version.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Version, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(VersionVisitor)
    }
}

// Option<Version> 的自定义序列化和反序列化
// 因为 Version 不能直接与 serde 一起使用
pub mod option_version_serde {
    use node_semver::Version;
    use serde::de::Error;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(version: &Option<Version>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match version {
            Some(v) => s.serialize_str(&v.to_string()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Version>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        if let Some(v) = s {
            return Ok(Some(
                Version::parse(super::trim_version(&v)).map_err(Error::custom)?,
            ));
        }
        Ok(None)
    }
}

// HashMap<String, Version> 的自定义反序列化
// 因为 Version 不能直接与 serde 一起使用
pub mod hashmap_version_serde {
    use super::version_serde;
    use node_semver::Version;
    use serde::{self, Deserialize, Deserializer};
    use std::collections::HashMap;

    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "version_serde::deserialize")] Version);

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<String, Version>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let m = HashMap::<String, Wrapper>::deserialize(deserializer)?;
        Ok(m.into_iter().map(|(k, Wrapper(v))| (k, v)).collect())
    }
}
