// 导入所需的模块和类型
use crate::error::{Context, ErrorKind, Fallible, VoltaError};
use crate::platform::PlatformSpec;
use crate::version::{option_version_serde, version_serde};
use node_semver::Version;
use serde::{Deserialize, Serialize};

// 定义 NodeVersion 结构体，用于序列化和反序列化
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct NodeVersion {
    #[serde(with = "version_serde")]
    pub runtime: Version,
    #[serde(with = "option_version_serde")]
    pub npm: Option<Version>,
}

// 定义 Platform 结构体，用于序列化和反序列化
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Platform {
    #[serde(default)]
    pub node: Option<NodeVersion>,
    #[serde(default)]
    #[serde(with = "option_version_serde")]
    pub pnpm: Option<Version>,
    #[serde(default)]
    #[serde(with = "option_version_serde")]
    pub yarn: Option<Version>,
}

impl Platform {
    // 从 PlatformSpec 创建 Platform 实例
    pub fn of(source: &PlatformSpec) -> Self {
        Platform {
            node: Some(NodeVersion {
                runtime: source.node.clone(),
                npm: source.npm.clone(),
            }),
            pnpm: source.pnpm.clone(),
            yarn: source.yarn.clone(),
        }
    }

    /// 将 Platform 序列化为 JSON 字符串
    pub fn into_json(self) -> Fallible<String> {
        serde_json::to_string_pretty(&self).with_context(|| ErrorKind::StringifyPlatformError)
    }
}

// 实现从字符串到 Platform 的转换
impl TryFrom<String> for Platform {
    type Error = VoltaError;
    fn try_from(src: String) -> Fallible<Self> {
        let result = if src.is_empty() {
            serde_json::de::from_str("{}")
        } else {
            serde_json::de::from_str(&src)
        };

        result.with_context(|| ErrorKind::ParsePlatformError)
    }
}

// 实现从 Platform 到 Option<PlatformSpec> 的转换
impl From<Platform> for Option<PlatformSpec> {
    fn from(platform: Platform) -> Option<PlatformSpec> {
        let yarn = platform.yarn;
        let pnpm = platform.pnpm;
        platform.node.map(|node_version| PlatformSpec {
            node: node_version.runtime,
            npm: node_version.npm,
            pnpm,
            yarn,
        })
    }
}

// 测试模块
#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::platform;
    use node_semver::Version;

    // 注意：serde_json 需要在 Cargo.toml 中启用 "preserve_order" 特性，
    // 以确保这些测试的序列化/反序列化顺序是可预测的

    const BASIC_JSON_STR: &str = r#"{
  "node": {
    "runtime": "4.5.6",
    "npm": "7.8.9"
  },
  "pnpm": "3.2.1",
  "yarn": "1.2.3"
}"#;

    // 测试从 JSON 字符串解析 Platform
    #[test]
    fn test_from_json() {
        let json_str = BASIC_JSON_STR.to_string();
        let platform = Platform::try_from(json_str).expect("could not parse JSON string");
        let expected_platform = Platform {
            pnpm: Some(Version::parse("3.2.1").expect("could not parse version")),
            yarn: Some(Version::parse("1.2.3").expect("could not parse version")),
            node: Some(NodeVersion {
                runtime: Version::parse("4.5.6").expect("could not parse version"),
                npm: Some(Version::parse("7.8.9").expect("could not parse version")),
            }),
        };
        assert_eq!(platform, expected_platform);
    }

    // 测试从空字符串解析 Platform
    #[test]
    fn test_from_json_empty_string() {
        let json_str = "".to_string();
        let platform = Platform::try_from(json_str).expect("could not parse JSON string");
        let expected_platform = Platform {
            node: None,
            pnpm: None,
            yarn: None,
        };
        assert_eq!(platform, expected_platform);
    }

    // 测试将 Platform 序列化为 JSON 字符串
    #[test]
    fn test_into_json() {
        let platform_spec = platform::PlatformSpec {
            pnpm: Some(Version::parse("3.2.1").expect("could not parse version")),
            yarn: Some(Version::parse("1.2.3").expect("could not parse version")),
            node: Version::parse("4.5.6").expect("could not parse version"),
            npm: Some(Version::parse("7.8.9").expect("could not parse version")),
        };
        let json_str = Platform::of(&platform_spec)
            .into_json()
            .expect("could not serialize platform to JSON");
        let expected_json_str = BASIC_JSON_STR.to_string();
        assert_eq!(json_str, expected_json_str);
    }
}
