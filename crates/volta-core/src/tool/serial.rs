use super::Spec;
use crate::error::{ErrorKind, Fallible};
use crate::version::{VersionSpec, VersionTag};
use log::info;
use once_cell::sync::Lazy;
use regex::Regex;
use std::cmp::Ordering;
use validate_npm_package_name::{validate, Validity};

// 定义用于匹配工具规格的正则表达式模式
static TOOL_SPEC_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new("^(?P<name>(?:@([^/]+?)[/])?([^/]+?))(@(?P<version>.+))?$").expect("regex is valid")
});

// 定义用于检查是否包含版本的正则表达式
static HAS_VERSION: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[^\s]+@").expect("regex is valid"));

/// Spec 的解析方法实现
impl Spec {
    // 从工具名称和版本创建 Spec
    pub fn from_str_and_version(tool_name: &str, version: VersionSpec) -> Self {
        match tool_name {
            "node" => Spec::Node(version),
            "npm" => Spec::Npm(version),
            "pnpm" => Spec::Pnpm(version),
            "yarn" => Spec::Yarn(version),
            package => Spec::Package(package.to_string(), version),
        }
    }

    /// 尝试从字符串解析工具和版本，格式为 `<tool>[@<version>]`
    pub fn try_from_str(tool_spec: &str) -> Fallible<Self> {
        let captures =
            TOOL_SPEC_PATTERN
                .captures(tool_spec)
                .ok_or_else(|| ErrorKind::ParseToolSpecError {
                    tool_spec: tool_spec.into(),
                })?;

        // 验证捕获的名称是否为有效的 NPM 包名
        let name = &captures["name"];
        if let Validity::Invalid { errors, .. } = validate(name) {
            return Err(ErrorKind::InvalidToolName {
                name: name.into(),
                errors,
            }
            .into());
        }

        let version = captures
            .name("version")
            .map(|version| version.as_str().parse())
            .transpose()?
            .unwrap_or_default();

        Ok(match name {
            "node" => Spec::Node(version),
            "npm" => Spec::Npm(version),
            "pnpm" => Spec::Pnpm(version),
            "yarn" => Spec::Yarn(version),
            package => Spec::Package(package.into(), version),
        })
    }

    /// 从字符串列表获取有效的、排序后的 `Vec<Spec>`
    ///
    /// 处理以下错误情况：
    ///
    /// - `volta install node 12`，用户本意是安装 `node@12`
    ///   但使用了类似 nodenv 或 nvm 的语法
    /// - 无效的版本规格
    ///
    /// 返回排序后的列表，如果包含 `node`，它始终在列表的首位
    pub fn from_strings<T>(tool_strs: &[T], action: &str) -> Fallible<Vec<Spec>>
    where
        T: AsRef<str>,
    {
        Self::check_args(tool_strs, action)?;

        let mut tools = tool_strs
            .iter()
            .map(|arg| Self::try_from_str(arg.as_ref()))
            .collect::<Fallible<Vec<Spec>>>()?;

        tools.sort_by(Self::sort_comparator);
        Ok(tools)
    }

    /// 检查参数是否存在以下错误模式：
    /// - `volta install <number>`
    /// - `volta install <tool> <number>`
    fn check_args<T>(args: &[T], action: &str) -> Fallible<()>
    where
        T: AsRef<str>,
    {
        let mut args = args.iter();

        match (args.next(), args.next(), args.next()) {
            // 这里关注的情况是只有一个参数，且该参数是有效的版本规格
            (Some(maybe_version), None, None) if is_version_like(maybe_version.as_ref()) => {
                Err(ErrorKind::InvalidInvocationOfBareVersion {
                    action: action.to_string(),
                    version: maybe_version.as_ref().to_string(),
                }
                .into())
            }
            // 这里关注的情况是有两个参数，第一个是工具名（不带 @version），第二个是有效的版本规格
            (Some(name), Some(maybe_version), None)
                if !HAS_VERSION.is_match(name.as_ref())
                    && is_version_like(maybe_version.as_ref()) =>
            {
                Err(ErrorKind::InvalidInvocation {
                    action: action.to_string(),
                    name: name.as_ref().to_string(),
                    version: maybe_version.as_ref().to_string(),
                }
                .into())
            }
            _ => Ok(()),
        }
    }

    /// 比较 `Spec` 以在从字符串转换时进行排序
    ///
    /// 我们尽可能保留原始顺序，所以我们将同一工具类别的工具视为相等。
    /// 我们仍然需要将 Node 放在列表的最前面，然后是 Npm、pnpm、Yarn，最后是 Packages。
    fn sort_comparator(left: &Spec, right: &Spec) -> Ordering {
        match (left, right) {
            (Spec::Node(_), Spec::Node(_)) => Ordering::Equal,
            (Spec::Node(_), _) => Ordering::Less,
            (_, Spec::Node(_)) => Ordering::Greater,
            (Spec::Npm(_), Spec::Npm(_)) => Ordering::Equal,
            (Spec::Npm(_), _) => Ordering::Less,
            (_, Spec::Npm(_)) => Ordering::Greater,
            (Spec::Pnpm(_), Spec::Pnpm(_)) => Ordering::Equal,
            (Spec::Pnpm(_), _) => Ordering::Less,
            (_, Spec::Pnpm(_)) => Ordering::Greater,
            (Spec::Yarn(_), Spec::Yarn(_)) => Ordering::Equal,
            (Spec::Yarn(_), _) => Ordering::Less,
            (_, Spec::Yarn(_)) => Ordering::Greater,
            (Spec::Package(_, _), Spec::Package(_, _)) => Ordering::Equal,
        }
    }
}

/// 判断给定的字符串是否"类似版本"
///
/// 这意味着它是 'latest'、'lts'、Version 或 Version Range
fn is_version_like(value: &str) -> bool {
    matches!(
        value.parse(),
        Ok(VersionSpec::Exact(_))
            | Ok(VersionSpec::Semver(_))
            | Ok(VersionSpec::Tag(VersionTag::Latest))
            | Ok(VersionSpec::Tag(VersionTag::Lts))
    )
}

#[cfg(test)]
mod tests {
    mod try_from_str {
        use std::str::FromStr as _;

        use super::super::super::Spec;
        use crate::version::{VersionSpec, VersionTag};

        const LTS: &str = "lts";
        const LATEST: &str = "latest";
        const MAJOR: &str = "3";
        const MINOR: &str = "3.0";
        const PATCH: &str = "3.0.0";
        const BETA: &str = "beta";

        /// 用于生成 <tool>@<version> 字符串的便捷宏
        macro_rules! versioned_tool {
            ($tool:expr, $version:expr) => {
                format!("{}@{}", $tool, $version)
            };
        }

        #[test]
        fn parses_bare_node() {
            assert_eq!(
                Spec::try_from_str("node").expect("succeeds"),
                Spec::Node(VersionSpec::default())
            );
        }

        #[test]
        fn parses_node_with_valid_versions() {
            let tool = "node";

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, MAJOR)).expect("succeeds"),
                Spec::Node(VersionSpec::from_str(MAJOR).expect("`VersionSpec` has its own tests"))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, MINOR)).expect("succeeds"),
                Spec::Node(VersionSpec::from_str(MINOR).expect("`VersionSpec` has its own tests"))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, PATCH)).expect("succeeds"),
                Spec::Node(VersionSpec::from_str(PATCH).expect("`VersionSpec` has its own tests"))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, LATEST)).expect("succeeds"),
                Spec::Node(VersionSpec::Tag(VersionTag::Latest))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, LTS)).expect("succeeds"),
                Spec::Node(VersionSpec::Tag(VersionTag::Lts))
            );
        }

        #[test]
        fn parses_bare_yarn() {
            assert_eq!(
                Spec::try_from_str("yarn").expect("succeeds"),
                Spec::Yarn(VersionSpec::default())
            );
        }

        #[test]
        fn parses_yarn_with_valid_versions() {
            let tool = "yarn";

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, MAJOR)).expect("succeeds"),
                Spec::Yarn(VersionSpec::from_str(MAJOR).expect("`VersionSpec` has its own tests"))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, MINOR)).expect("succeeds"),
                Spec::Yarn(VersionSpec::from_str(MINOR).expect("`VersionSpec` has its own tests"))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, PATCH)).expect("succeeds"),
                Spec::Yarn(VersionSpec::from_str(PATCH).expect("`VersionSpec` has its own tests"))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(tool, LATEST)).expect("succeeds"),
                Spec::Yarn(VersionSpec::Tag(VersionTag::Latest))
            );
        }

        #[test]
        fn parses_bare_packages() {
            let package = "ember-cli";
            assert_eq!(
                Spec::try_from_str(package).expect("succeeds"),
                Spec::Package(package.into(), VersionSpec::default())
            );
        }

        #[test]
        fn parses_namespaced_packages() {
            let package = "@types/lodash";
            assert_eq!(
                Spec::try_from_str(package).expect("succeeds"),
                Spec::Package(package.into(), VersionSpec::default())
            );
        }

        #[test]
        fn parses_bare_packages_with_valid_versions() {
            let package = "something-awesome";

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, MAJOR)).expect("succeeds"),
                Spec::Package(
                    package.into(),
                    VersionSpec::from_str(MAJOR).expect("`VersionSpec` has its own tests")
                )
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, MINOR)).expect("succeeds"),
                Spec::Package(
                    package.into(),
                    VersionSpec::from_str(MINOR).expect("`VersionSpec` has its own tests")
                )
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, PATCH)).expect("succeeds"),
                Spec::Package(
                    package.into(),
                    VersionSpec::from_str(PATCH).expect("`VersionSpec` has its own tests")
                )
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, LATEST)).expect("succeeds"),
                Spec::Package(package.into(), VersionSpec::Tag(VersionTag::Latest))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, LTS)).expect("succeeds"),
                Spec::Package(package.into(), VersionSpec::Tag(VersionTag::Lts))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, BETA)).expect("succeeds"),
                Spec::Package(
                    package.into(),
                    VersionSpec::Tag(VersionTag::Custom(BETA.into()))
                )
            );
        }

        #[test]
        fn parses_namespaced_packages_with_valid_versions() {
            let package = "@something/awesome";

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, MAJOR)).expect("succeeds"),
                Spec::Package(
                    package.into(),
                    VersionSpec::from_str(MAJOR).expect("`VersionSpec` has its own tests")
                )
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, MINOR)).expect("succeeds"),
                Spec::Package(
                    package.into(),
                    VersionSpec::from_str(MINOR).expect("`VersionSpec` has its own tests")
                )
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, PATCH)).expect("succeeds"),
                Spec::Package(
                    package.into(),
                    VersionSpec::from_str(PATCH).expect("`VersionSpec` has its own tests")
                )
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, LATEST)).expect("succeeds"),
                Spec::Package(package.into(), VersionSpec::Tag(VersionTag::Latest))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, LTS)).expect("succeeds"),
                Spec::Package(package.into(), VersionSpec::Tag(VersionTag::Lts))
            );

            assert_eq!(
                Spec::try_from_str(&versioned_tool!(package, BETA)).expect("succeeds"),
                Spec::Package(
                    package.into(),
                    VersionSpec::Tag(VersionTag::Custom(BETA.into()))
                )
            );
        }
    }

    mod from_strings {
        use super::super::*;
        use std::str::FromStr;

        static PIN: &str = "pin";

        #[test]
        fn special_cases_just_number() {
            let version = "1.2.3";
            let args: Vec<String> = vec![version.into()];

            let err = Spec::from_strings(&args, PIN).unwrap_err();

            assert_eq!(
                err.kind(),
                &ErrorKind::InvalidInvocationOfBareVersion {
                    action: PIN.into(),
                    version: version.into()
                },
                "`volta <action> number` results in the correct error"
            );
        }

        #[test]
        fn special_cases_tool_space_number() {
            let name = "potato";
            let version = "1.2.3";
            let args: Vec<String> = vec![name.into(), version.into()];

            let err = Spec::from_strings(&args, PIN).unwrap_err();

            assert_eq!(
                err.kind(),
                &ErrorKind::InvalidInvocation {
                    action: PIN.into(),
                    name: name.into(),
                    version: version.into()
                },
                "`volta <action> tool number` results in the correct error"
            );
        }

        #[test]
        fn leaves_other_scenarios_alone() {
            let empty: Vec<&str> = Vec::new();
            assert_eq!(
                Spec::from_strings(&empty, PIN).expect("is ok").len(),
                empty.len(),
                "when there are no args"
            );

            let only_one = ["node".to_owned()];
            assert_eq!(
                Spec::from_strings(&only_one, PIN).expect("is ok").len(),
                only_one.len(),
                "when there is only one arg"
            );

            let one_with_explicit_verson = ["10@latest".to_owned()];
            assert_eq!(
                Spec::from_strings(&one_with_explicit_verson, PIN)
                    .expect("is ok")
                    .len(),
                only_one.len(),
                "when the sole arg is version-like but has an explicit version"
            );

            let two_but_unmistakable = ["12".to_owned(), "node".to_owned()];
            assert_eq!(
                Spec::from_strings(&two_but_unmistakable, PIN)
                    .expect("is ok")
                    .len(),
                two_but_unmistakable.len(),
                "when there are two args but the order is not likely to be a mistake"
            );

            let two_but_valid_first = ["node@lts".to_owned(), "12".to_owned()];
            assert_eq!(
                Spec::from_strings(&two_but_valid_first, PIN)
                    .expect("is ok")
                    .len(),
                two_but_valid_first.len(),
                "when there are two args but the first is a valid tool spec"
            );

            let more_than_two_tools = ["node".to_owned(), "12".to_owned(), "yarn".to_owned()];
            assert_eq!(
                Spec::from_strings(&more_than_two_tools, PIN)
                    .expect("is ok")
                    .len(),
                more_than_two_tools.len(),
                "when there are more than two args"
            );
        }

        #[test]
        fn sorts_node_npm_yarn_to_front() {
            let multiple = [
                "ember-cli@3".to_owned(),
                "yarn".to_owned(),
                "npm@5".to_owned(),
                "node@latest".to_owned(),
            ];
            let expected = [
                Spec::Node(VersionSpec::Tag(VersionTag::Latest)),
                Spec::Npm(VersionSpec::from_str("5").expect("requirement is valid")),
                Spec::Yarn(VersionSpec::default()),
                Spec::Package(
                    "ember-cli".to_owned(),
                    VersionSpec::from_str("3").expect("requirement is valid"),
                ),
            ];
            assert_eq!(Spec::from_strings(&multiple, PIN).expect("is ok"), expected);
        }

        #[test]
        fn keeps_package_order_unchanged() {
            let packages_with_node = ["typescript@latest", "ember-cli@3", "node@lts", "mocha"];
            let expected = [
                Spec::Node(VersionSpec::Tag(VersionTag::Lts)),
                Spec::Package(
                    "typescript".to_owned(),
                    VersionSpec::Tag(VersionTag::Latest),
                ),
                Spec::Package(
                    "ember-cli".to_owned(),
                    VersionSpec::from_str("3").expect("requirement is valid"),
                ),
                Spec::Package("mocha".to_owned(), VersionSpec::default()),
            ];

            assert_eq!(
                Spec::from_strings(&packages_with_node, PIN).expect("is ok"),
                expected
            );
        }
    }
}
