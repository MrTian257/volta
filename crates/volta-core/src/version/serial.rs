use node_semver::{Range, SemverError};

// 注意：这里使用 `parse_compat` 是因为 semver crate 默认以 cargo 兼容的方式解析。
// 这通常没问题，除了两种情况（据我所知）：
//  * "1.2.3" 对于 cargo 解析为 `^1.2.3`，但对于 Node 解析为 `=1.2.3`
//  * `>1.2.3 <2.0.0` 对于 cargo 序列化为 ">1.2.3, <2.0.0"（带逗号），
//    但对于 Node 序列化为 ">1.2.3 <2.0.0"（无逗号，因为 Node 对逗号的解析不同）
//
// 因为我们从命令行解析版本要求，然后序列化它们以传递给 `npm view`，
// 所以需要以 Node 兼容的方式处理它们（否则会返回错误的版本信息）。

// 解析版本要求
pub fn parse_requirements(src: &str) -> Result<Range, SemverError> {
    // 去除首尾空白和开头的 'v' 字符
    let src = src.trim().trim_start_matches('v');

    // 使用 Node 兼容的方式解析版本范围
    Range::parse(src)
}

#[cfg(test)]
pub mod tests {

    use crate::version::serial::parse_requirements;
    use node_semver::Range;

    #[test]
    fn test_parse_requirements() {
        // 测试各种版本格式的解析
        assert_eq!(
            parse_requirements("1.2.3").unwrap(),
            Range::parse("=1.2.3").unwrap()
        );
        assert_eq!(
            parse_requirements("v1.5").unwrap(),
            Range::parse("=1.5").unwrap()
        );
        assert_eq!(
            parse_requirements("=1.2.3").unwrap(),
            Range::parse("=1.2.3").unwrap()
        );
        assert_eq!(
            parse_requirements("^1.2").unwrap(),
            Range::parse("^1.2").unwrap()
        );
        assert_eq!(
            parse_requirements(">=1.4").unwrap(),
            Range::parse(">=1.4").unwrap()
        );
        assert_eq!(
            parse_requirements("8.11 - 8.17 || 10.* || >= 12").unwrap(),
            Range::parse("8.11 - 8.17 || 10.* || >= 12").unwrap()
        );
    }
}
