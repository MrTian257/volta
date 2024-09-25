use std::fmt;
use std::path::PathBuf;

use super::ExitCode;
use crate::style::{text_width, tool_version};
use crate::tool;
use crate::tool::package::PackageManager;
use textwrap::{fill, indent};

// 报告错误的提示信息
// Call to action to report a bug
const REPORT_BUG_CTA: &str =
    "请使用环境变量 `VOLTA_LOGLEVEL` 设置为 `debug` 重新运行触发此错误的命令，
并在 https://github.com/volta-cli/volta/issues 上提交一个包含详细信息的问题！";

// 权限相关的提示信息
// Call to action for permission issues
const PERMISSIONS_CTA: &str = "请确保您对 Volta 目录具有正确的权限。";

// 错误类型枚举
// Enum of error kinds
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum ErrorKind {
    // 当包尝试安装已经安装的二进制文件时抛出
    /// Thrown when package tries to install a binary that is already installed.
    BinaryAlreadyInstalled {
        bin_name: String,
        existing_package: String,
        new_package: String,
    },

    // 当执行外部二进制文件失败时抛出
    /// Thrown when executing an external binary fails
    BinaryExecError,

    // 当在本地库中找不到二进制文件时抛出
    /// Thrown when a binary could not be found in the local inventory
    BinaryNotFound {
        name: String,
    },

    // 当构建虚拟环境路径失败时抛出
    /// Thrown when building the virtual environment path fails
    BuildPathError,

    // 当无法使用 VOLTA_BYPASS 启动命令时抛出
    /// Thrown when unable to launch a command with VOLTA_BYPASS set
    BypassError {
        command: String,
    },

    // 当用户尝试 `volta fetch` node/yarn/npm 以外的内容时抛出
    /// Thrown when a user tries to `volta fetch` something other than node/yarn/npm.
    CannotFetchPackage {
        package: String,
    },

    // 当用户尝试 `volta pin` node/yarn/npm 以外的内容时抛出
    /// Thrown when a user tries to `volta pin` something other than node/yarn/npm.
    CannotPinPackage {
        package: String,
    },

    // 当 Completions 输出目录不是一个目录时抛出
    /// Thrown when the Completions out-dir is not a directory
    CompletionsOutFileError {
        path: PathBuf,
    },

    // 当无法确定包含目录时抛出
    /// Thrown when the containing directory could not be determined
    ContainingDirError {
        path: PathBuf,
    },

    // 当无法确定工具时抛出
    CouldNotDetermineTool,

    // 当无法启动迁移可执行文件时抛出
    /// Thrown when unable to start the migration executable
    CouldNotStartMigration,

    // 当创建目录失败时抛出
    CreateDirError {
        dir: PathBuf,
    },

    // 当无法创建布局文件时抛出
    /// Thrown when unable to create the layout file
    CreateLayoutFileError {
        file: PathBuf,
    },

    // 当无法创建到共享全局库目录的链接时抛出
    /// Thrown when unable to create a link to the shared global library directory
    CreateSharedLinkError {
        name: String,
    },

    // 当创建临时目录失败时抛出
    /// Thrown when creating a temporary directory fails
    CreateTempDirError {
        in_dir: PathBuf,
    },

    // 当创建临时文件失败时抛出
    /// Thrown when creating a temporary file fails
    CreateTempFileError {
        in_dir: PathBuf,
    },

    // 当无法确定当前目录时抛出
    CurrentDirError,

    // 当删除目录失败时抛出
    /// Thrown when deleting a directory fails
    DeleteDirectoryError {
        directory: PathBuf,
    },

    // 当删除文件失败时抛出
    /// Thrown when deleting a file fails
    DeleteFileError {
        file: PathBuf,
    },

    // 当使用已弃用的命令时抛出
    DeprecatedCommandError {
        command: String,
        advice: String,
    },

    // 当下载工具网络错误时抛出
    DownloadToolNetworkError {
        tool: tool::Spec,
        from_url: String,
    },

    // 当无法执行钩子命令时抛出
    /// Thrown when unable to execute a hook command
    ExecuteHookError {
        command: String,
    },

    // 当 `volta.extends` 键导致无限循环时抛出
    /// Thrown when `volta.extends` keys result in an infinite cycle
    ExtensionCycleError {
        paths: Vec<PathBuf>,
        duplicate: PathBuf,
    },

    // 当确定扩展清单的路径失败时抛出
    /// Thrown when determining the path to an extension manifest fails
    ExtensionPathError {
        path: PathBuf,
    },

    // 当钩子命令返回非零退出代码时抛出
    /// Thrown when a hook command returns a non-zero exit code
    HookCommandFailed {
        command: String,
    },

    // 当钩子包含多个字段（prefix、template 或 bin）时抛出
    /// Thrown when a hook contains multiple fields (prefix, template, or bin)
    HookMultipleFieldsSpecified,

    // 当钩子不包含任何已知字段（prefix、template 或 bin）时抛出
    /// Thrown when a hook doesn't contain any of the known fields (prefix, template, or bin)
    HookNoFieldsSpecified,

    // 当确定钩子的路径失败时抛出
    /// Thrown when determining the path to a hook fails
    HookPathError {
        command: String,
    },

    // 当确定新安装包的名称失败时抛出
    /// Thrown when determining the name of a newly-installed package fails
    InstalledPackageNameError,

    // 当钩子命令无效时抛出
    InvalidHookCommand {
        command: String,
    },

    // 当无法读取钩子命令的输出时抛出
    /// Thrown when output from a hook command could not be read
    InvalidHookOutput {
        command: String,
    },

    // 当用户执行如 `volta install node 12` 而不是 `volta install node@12` 时抛出
    /// Thrown when a user does e.g. `volta install node 12` instead of
    /// `volta install node@12`.
    InvalidInvocation {
        action: String,
        name: String,
        version: String,
    },

    // 当用户执行如 `volta install 12` 而不是 `volta install node@12` 时抛出
    /// Thrown when a user does e.g. `volta install 12` instead of
    /// `volta install node@12`.
    InvalidInvocationOfBareVersion {
        action: String,
        version: String,
    },

    // 当在钩子中给出的 yarn.index 格式不是 "npm" 或 "github" 时抛出
    /// Thrown when a format other than "npm" or "github" is given for yarn.index in the hooks
    InvalidRegistryFormat {
        format: String,
    },

    // 当工具名称根据 npm 的规则无效时抛出
    /// Thrown when a tool name is invalid per npm's rules.
    InvalidToolName {
        name: String,
        errors: Vec<String>,
    },

    // 当无法获取 Volta 目录的锁时抛出
    /// Thrown when unable to acquire a lock on the Volta directory
    LockAcquireError,

    // 当固定或安装 npm@bundled 并且无法检测到捆绑版本时抛出
    /// Thrown when pinning or installing npm@bundled and couldn't detect the bundled version
    NoBundledNpm {
        command: String,
    },

    // 当命令行中未设置 pnpm 时抛出
    /// Thrown when pnpm is not set at the command-line
    NoCommandLinePnpm,

    // 当命令行中未设置 Yarn 时抛出
    /// Thrown when Yarn is not set at the command-line
    NoCommandLineYarn,

    // 当用户在安装 Node 版本之前尝试安装 Yarn 或 npm 版本时抛出
    /// Thrown when a user tries to install a Yarn or npm version before installing a Node version.
    NoDefaultNodeVersion {
        tool: String,
    },

    // 当没有 Node 版本匹配请求的语义版本说明符时抛出
    /// Thrown when there is no Node version matching a requested semver specifier.
    NodeVersionNotFound {
        matching: String,
    },

    // 当没有 HOME 环境变量时抛出
    NoHomeEnvironmentVar,

    // 当无法确定安装目录时抛出
    /// Thrown when the install dir could not be determined
    NoInstallDir,

    // 当没有本地数据目录时抛出
    NoLocalDataDir,

    // 当用户在固定 Node 版本之前尝试固定 npm、pnpm 或 Yarn 版本时抛出
    /// Thrown when a user tries to pin a npm, pnpm, or Yarn version before pinning a Node version.
    NoPinnedNodeVersion {
        tool: String,
    },

    // 当无法确定平台（Node 版本）时抛出
    /// Thrown when the platform (Node version) could not be determined
    NoPlatform,

    // 当解析项目清单时存在 `"volta"` 键但没有 Node 时抛出
    /// Thrown when parsing the project manifest and there is a `"volta"` key without Node
    NoProjectNodeInManifest,

    // 当项目中未设置 Yarn 时抛出
    /// Thrown when Yarn is not set in a project
    NoProjectYarn,

    // 当项目中未设置 pnpm 时抛出
    /// Thrown when pnpm is not set in a project
    NoProjectPnpm,

    // 当找不到 shell 配置文件时抛出
    /// Thrown when no shell profiles could be found
    NoShellProfile {
        env_profile: String,
        bin_dir: PathBuf,
    },

    // 当用户尝试在包外固定 Node 或 Yarn 版本时抛出
    /// Thrown when the user tries to pin Node or Yarn versions outside of a package.
    NotInPackage,

    // 当未设置默认 Yarn 时抛出
    /// Thrown when default Yarn is not set
    NoDefaultYarn,

    // 当未设置默认 pnpm 时抛出
    /// Thrown when default pnpm is not set
    NoDefaultPnpm,

    // 当使用不可用的包调用 `npm link` 时抛出
    /// Thrown when `npm link` is called with a package that isn't available
    NpmLinkMissingPackage {
        package: String,
    },

    // 当使用未通过 npm 安装/链接的包调用 `npm link` 时抛出
    /// Thrown when `npm link` is called with a package that was not installed / linked with npm
    NpmLinkWrongManager {
        package: String,
    },

    // 当没有 npm 版本匹配请求的语义版本/标签时抛出
    /// Thrown when there is no npm version matching the requested Semver/Tag
    NpmVersionNotFound {
        matching: String,
    },

    // 当 npx 不可用时抛出
    NpxNotAvailable {
        version: String,
    },

    // 当安装全局包的命令不成功时抛出
    /// Thrown when the command to install a global package is not successful
    PackageInstallFailed {
        package: String,
    },

    // 当解析包清单失败时抛出
    /// Thrown when parsing the package manifest fails
    PackageManifestParseError {
        package: String,
    },

    // 当读取包清单失败时抛出
    /// Thrown when reading the package manifest fails
    PackageManifestReadError {
        package: String,
    },

    // 当在 npm 注册表中找不到指定的包时抛出
    /// Thrown when a specified package could not be found on the npm registry
    PackageNotFound {
        package: String,
    },

    // 当解析包清单失败时抛出
    /// Thrown when parsing a package manifest fails
    PackageParseError {
        file: PathBuf,
    },

    // 当读取包清单失败时抛出
    /// Thrown when reading a package manifest fails
    PackageReadError {
        file: PathBuf,
    },

    // 当包已解压但格式不正确时抛出
    /// Thrown when a package has been unpacked but is not formed correctly.
    PackageUnpackError,

    // 当写入包清单失败时抛出
    /// Thrown when writing a package manifest fails
    PackageWriteError {
        file: PathBuf,
    },

    // 当无法解析 bin 配置文件时抛出
    /// Thrown when unable to parse a bin config file
    ParseBinConfigError,

    // 当无法解析 hooks.json 文件时抛出
    /// Thrown when unable to parse a hooks.json file
    ParseHooksError {
        file: PathBuf,
    },

    // 当无法解析 node 索引缓存时抛出
    /// Thrown when unable to parse the node index cache
    ParseNodeIndexCacheError,

    // 当无法解析 node 索引时抛出
    /// Thrown when unable to parse the node index
    ParseNodeIndexError {
        from_url: String,
    },

    // 当无法解析 node 索引缓存过期时间时抛出
    /// Thrown when unable to parse the node index cache expiration
    ParseNodeIndexExpiryError,

    // 当无法解析 node 安装中的 npm 清单文件时抛出
    /// Thrown when unable to parse the npm manifest file from a node install
    ParseNpmManifestError,

    // 当无法解析包配置时抛出
    /// Thrown when unable to parse a package configuration
    ParsePackageConfigError,

    // 当无法解析 platform.json 文件时抛出
    /// Thrown when unable to parse the platform.json file
    ParsePlatformError,

    // 当无法解析工具规格（`<tool>[@<version>]`）时抛出
    /// Thrown when unable to parse a tool spec (`<tool>[@<version>]`)
    ParseToolSpecError {
        tool_spec: String,
    },

    // 当将归档持久化到库存失败时抛出
    /// Thrown when persisting an archive to the inventory fails
    PersistInventoryError {
        tool: String,
    },

    // 当没有 pnpm 版本匹配请求的语义版本说明符时抛出
    /// Thrown when there is no pnpm version matching a requested semver specifier.
    PnpmVersionNotFound {
        matching: String,
    },

    // 当执行项目本地二进制文件失败时抛出
    /// Thrown when executing a project-local binary fails
    ProjectLocalBinaryExecError {
        command: String,
    },

    // 当找不到项目本地二进制文件时抛出
    /// Thrown when a project-local binary could not be found
    ProjectLocalBinaryNotFound {
        command: String,
    },

    // 当发布钩子同时包含 url 和 bin 字段时抛出
    /// Thrown when a publish hook contains both the url and bin fields
    PublishHookBothUrlAndBin,

    // 当发布钩子既不包含 url 也不包含 bin 字段时抛出
    /// Thrown when a publish hook contains neither url nor bin fields
    PublishHookNeitherUrlNorBin,

    // 当读取用户 bin 目录时出错时抛出
    /// Thrown when there was an error reading the user bin directory
    ReadBinConfigDirError {
        dir: PathBuf,
    },

    // 当读取二进制文件的配置时出错时抛出
    /// Thrown when there was an error reading the config for a binary
    ReadBinConfigError {
        file: PathBuf,
    },

    // 当无法读取默认 npm 版本文件时抛出
    /// Thrown when unable to read the default npm version file
    ReadDefaultNpmError {
        file: PathBuf,
    },

    // 当无法读取目录内容时抛出
    /// Thrown when unable to read the contents of a directory
    ReadDirError {
        dir: PathBuf,
    },

    // 当打开 hooks.json 文件时出错时抛出
    /// Thrown when there was an error opening a hooks.json file
    ReadHooksError {
        file: PathBuf,
    },

    // 当读取 Node 索引缓存时出错时抛出
    /// Thrown when there was an error reading the Node Index Cache
    ReadNodeIndexCacheError {
        file: PathBuf,
    },

    // 当读取 Node 索引缓存过期时间时出错时抛出
    /// Thrown when there was an error reading the Node Index Cache Expiration
    ReadNodeIndexExpiryError {
        file: PathBuf,
    },

    // 当读取 npm 清单文件时出错时抛出
    /// Thrown when there was an error reading the npm manifest file
    ReadNpmManifestError,

    // 当读取包配置文件时出错时抛出
    /// Thrown when there was an error reading a package configuration file
    ReadPackageConfigError {
        file: PathBuf,
    },

    // 当打开用户平台文件时出错时抛出
    /// Thrown when there was an error opening the user platform file
    ReadPlatformError {
        file: PathBuf,
    },

    // 当无法从注册表读取用户 Path 环境变量时抛出
    /// Thrown when unable to read the user Path environment variable from the registry
    #[cfg(windows)]
    ReadUserPathError,

    // 当无法下载 Node 或 Yarn 的公共注册表时抛出
    /// Thrown when the public registry for Node or Yarn could not be downloaded.
    RegistryFetchError {
        tool: String,
        from_url: String,
    },

    // 当直接调用 shim 二进制文件而不是通过符号链接时抛出
    /// Thrown when the shim binary is called directly, not through a symlink
    RunShimDirectly,

    // 当设置工具为可执行时出错时抛出
    /// Thrown when there was an error setting a tool to executable
    SetToolExecutable {
        tool: String,
    },

    // 当将解压的工具复制到镜像目录时出错时抛出
    /// Thrown when there was an error copying an unpacked tool to the image directory
    SetupToolImageError {
        tool: String,
        version: String,
        dir: PathBuf,
    },

    // 当 Volta 无法创建 shim 时抛出
    /// Thrown when Volta is unable to create a shim
    ShimCreateError {
        name: String,
    },

    // 当 Volta 无法删除 shim 时抛出
    /// Thrown when Volta is unable to remove a shim
    ShimRemoveError {
        name: String,
    },

    // 当将 bin 配置序列化为 JSON 失败时抛出
    /// Thrown when serializing a bin config to JSON fails
    StringifyBinConfigError,

    // 当将包配置序列化为 JSON 失败时抛出
    /// Thrown when serializing a package config to JSON fails
    StringifyPackageConfigError,

    // 当将平台序列化为 JSON 失败时抛出
    /// Thrown when serializing the platform to JSON fails
    StringifyPlatformError,

    // 当给定的功能尚未实现时抛出
    /// Thrown when a given feature has not yet been implemented
    Unimplemented {
        feature: String,
    },

    // 当解压归档（tarball 或 zip）失败时抛出
    /// Thrown when unpacking an archive (tarball or zip) fails
    UnpackArchiveError {
        tool: String,
        version: String,
    },

    // 当找不到要升级的包时抛出
    /// Thrown when a package to upgrade was not found
    UpgradePackageNotFound {
        package: String,
        manager: PackageManager,
    },

    // 当要升级的包是用不同的包管理器安装的时抛出
    /// Thrown when a package to upgrade was installed with a different package manager
    UpgradePackageWrongManager {
        package: String,
        manager: PackageManager,
    },

    // 当版本解析错误时抛出
    VersionParseError {
        version: String,
    },

    // 当写入 bin 配置文件时出错时抛出
    /// Thrown when there was an error writing a bin config file
    WriteBinConfigError {
        file: PathBuf,
    },

    // 当写入默认 npm 到文件时出错时抛出
    /// Thrown when there was an error writing the default npm to file
    WriteDefaultNpmError {
        file: PathBuf,
    },

    // 当写入 npm 启动器时出错时抛出
    /// Thrown when there was an error writing the npm launcher
    WriteLauncherError {
        tool: String,
    },

    // 当写入 node 索引缓存时出错时抛出
    /// Thrown when there was an error writing the node index cache
    WriteNodeIndexCacheError {
        file: PathBuf,
    },

    // 当写入 node 索引过期时间时出错时抛出
    /// Thrown when there was an error writing the node index expiration
    WriteNodeIndexExpiryError {
        file: PathBuf,
    },

    // 当写入包配置时出错时抛出
    /// Thrown when there was an error writing a package config
    WritePackageConfigError {
        file: PathBuf,
    },

    // 当写入 platform.json 文件失败时抛出
    /// Thrown when writing the platform.json file fails
    WritePlatformError {
        file: PathBuf,
    },

    // 当无法写入用户 PATH 环境变量时抛出
    /// Thrown when unable to write the user PATH environment variable
    #[cfg(windows)]
    WriteUserPathError,

    // 当用户尝试安装 Yarn2 版本时抛出
    /// Thrown when a user attempts to install a version of Yarn2
    Yarn2NotSupported,

    // 当获取最新版本的 Yarn 时出错时抛出
    /// Thrown when there is an error fetching the latest version of Yarn
    YarnLatestFetchError {
        from_url: String,
    },

    // 当没有 Yarn 版本匹配请求的语义版本说明符时抛出
    /// Thrown when there is no Yarn version matching a requested semver specifier.
    YarnVersionNotFound {
        matching: String,
    },
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::BinaryAlreadyInstalled {
                bin_name,
                existing_package,
                new_package,
            } => write!(
                f,
                "可执行文件 '{}' 已经由 {} 安装

请在安装 {} 之前移除 {}",
                bin_name, existing_package, new_package, existing_package
            ),
            ErrorKind::BinaryExecError => write!(
                f,
                "无法执行命令。

请查看 `volta help install` 和 `volta help pin` 以了解如何使工具可用。"
            ),
            ErrorKind::BinaryNotFound { name } => write!(
                f,
                r#"找不到可执行文件 "{}"

使用 `volta install` 将包添加到您的工具链中（更多信息请参见 `volta help install`）。"#,
                name
            ),
            ErrorKind::BuildPathError => write!(
                f,
                "无法创建执行环境。

请确保您的 PATH 有效。"
            ),
            ErrorKind::BypassError { command } => write!(
                f,
                "无法执行命令 '{}'

VOLTA_BYPASS 已启用，请确保该命令存在于您的系统中或取消设置 VOLTA_BYPASS",
                command,
            ),
            ErrorKind::CannotFetchPackage { package } => write!(
                f,
                "不支持在不安装的情况下获取包。

使用 `volta install {}` 更新默认版本。",
                package
            ),
            ErrorKind::CannotPinPackage { package } => write!(
                f,
                "只能在项目中固定 node 和 yarn

使用 `npm install` 或 `yarn add` 为此项目选择 {} 的版本。",
                package
            ),
            ErrorKind::CompletionsOutFileError { path } => write!(
                f,
                "补全文件 `{}` 已存在。

请删除该文件或传递 `-f` 或 `--force` 以覆盖。",
                path.display()
            ),
            ErrorKind::ContainingDirError { path } => write!(
                f,
                "无法创建 {} 的包含目录

{}",
                path.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::CouldNotDetermineTool => write!(
                f,
                "无法确定工具名称

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::CouldNotStartMigration => write!(
                f,
                "无法启动迁移过程以升级您的 Volta 目录。

请确保您的 PATH 中有 'volta-migrate' 并直接运行它。"
            ),
            ErrorKind::CreateDirError { dir } => write!(
                f,
                "无法创建目录 {}

请确保您有正确的权限。",
                dir.display()
            ),
            ErrorKind::CreateLayoutFileError { file } => write!(
                f,
                "无法创建布局文件 {}

{}",
                file.display(), PERMISSIONS_CTA
            ),
            ErrorKind::CreateSharedLinkError { name } => write!(
                f,
                "无法为包 '{}' 创建共享环境

{}",
                name, PERMISSIONS_CTA
            ),
            ErrorKind::CreateTempDirError { in_dir } => write!(
                f,
                "无法创建临时目录
在 {}

{}",
                in_dir.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::CreateTempFileError { in_dir } => write!(
                f,
                "无法创建临时文件
在 {}

{}",
                in_dir.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::CurrentDirError => write!(
                f,
                "无法确定当前目录

请确保您有正确的权限。"
            ),
            ErrorKind::DeleteDirectoryError { directory } => write!(
                f,
                "无法删除目录
在 {}

{}",
                directory.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::DeleteFileError { file } => write!(
                f,
                "无法删除文件
在 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::DeprecatedCommandError { command, advice } => {
                write!(f, "子命令 `{}` 已被弃用。\n{}", command, advice)
            }
            ErrorKind::DownloadToolNetworkError { tool, from_url } => write!(
                f,
                "无法下载 {}
从 {}

请验证您的互联网连接并确保指定了正确的版本。",
                tool, from_url
            ),
            ErrorKind::ExecuteHookError { command } => write!(
                f,
                "无法执行钩子命令：'{}'

请确保指定了正确的命令。",
                command
            ),
            ErrorKind::ExtensionCycleError { paths, duplicate } => {
                // 在项目工作空间中检测到无限循环：
                //
                // --> /home/user/workspace/project/package.json
                //     /home/user/workspace/package.json
                // --> /home/user/workspace/project/package.json
                //
                // 请确保项目工作空间不相互依赖。
                f.write_str("在项目工作空间中检测到无限循环：\n\n")?;

                for path in paths {
                    if path == duplicate {
                        f.write_str("--> ")?;
                    } else {
                        f.write_str("    ")?;
                    }

                    writeln!(f, "{}", path.display())?;
                }

                writeln!(f, "--> {}", duplicate.display())?;
                writeln!(f)?;

                f.write_str("请确保项目工作空间不相互依赖。")
            }
            ErrorKind::ExtensionPathError { path } => write!(
                f,
                "无法确定项目工作空间的路径：'{}'

请确保文件存在且可访问。",
                path.display(),
            ),
            ErrorKind::HookCommandFailed { command } => write!(
                f,
                "钩子命令 '{}' 指示失败。

请验证请求的工具和版本。",
                command
            ),
            ErrorKind::HookMultipleFieldsSpecified => write!(
                f,
                "钩子配置包含多个钩子类型。

请只包含 'bin'、'prefix' 或 'template' 中的一个"
            ),
            ErrorKind::HookNoFieldsSpecified => write!(
                f,
                "钩子配置不包含任何钩子类型。

请包含 'bin'、'prefix' 或 'template' 中的一个"
            ),
            ErrorKind::HookPathError { command } => write!(
                f,
                "无法确定钩子命令的路径：'{}'

请确保指定了正确的命令。",
                command
            ),
            ErrorKind::InstalledPackageNameError => write!(
                f,
                "无法确定刚刚安装的包的名称。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::InvalidHookCommand { command } => write!(
                f,
                "无效的钩子命令：'{}'

请确保指定了正确的命令。",
                command
            ),
            ErrorKind::InvalidHookOutput { command } => write!(
                f,
                "无法读取钩子命令的输出：'{}'

请确保命令输出是有效的 UTF-8 文本。",
                command
            ),

            ErrorKind::InvalidInvocation {
                action,
                name,
                version,
            } => {
                let error = format!(
                    "不支持 `volta {action} {name} {version}`。",
                    action = action,
                    name = name,
                    version = version
                );

                let call_to_action = format!(
"要 {action} '{name}' 版本 '{version}'，请运行 `volta {action} {formatted}`。 \
要 {action} 包 '{name}' 和 '{version}'，请在单独的命令中 {action} 它们，或使用显式版本。",
                    action=action,
                    name=name,
                    version=version,
                    formatted=tool_version(name, version)
                );

                let wrapped_cta = match text_width() {
                    Some(width) => fill(&call_to_action, width),
                    None => call_to_action,
                };

                write!(f, "{}\n\n{}", error, wrapped_cta)
            }

            ErrorKind::InvalidInvocationOfBareVersion {
                action,
                version,
            } => {
                let error = format!(
                    "不支持 `volta {action} {version}`。",
                    action = action,
                    version = version
                );

                let call_to_action = format!(
"要 {action} node 版本 '{version}'，请运行 `volta {action} {formatted}`。 \
要 {action} 包 '{version}'，请使用显式版本，如 '{version}@latest'。",
                    action=action,
                    version=version,
                    formatted=tool_version("node", version)
                );

                let wrapped_cta = match text_width() {
                    Some(width) => fill(&call_to_action, width),
                    None => call_to_action,
                };

                write!(f, "{}\n\n{}", error, wrapped_cta)
            }

            ErrorKind::InvalidRegistryFormat { format } => write!(
                f,
                "无法识别的索引注册表格式：'{}'

请为格式指定 'npm' 或 'github'。",
format
            ),

            ErrorKind::InvalidToolName { name, errors } => {
                let indentation = "    ";
                let wrapped = match text_width() {
                    Some(width) => fill(&errors.join("\n"), width - indentation.len()),
                    None => errors.join("\n"),
                };
                let formatted_errs = indent(&wrapped, indentation);

                let call_to_action = if errors.len() > 1 {
                    "请修复以下错误："
                } else {
                    "请修复以下错误："
                };

                write!(
                    f,
                    "无效的工具名称 `{}`\n\n{}\n{}",
                    name, call_to_action, formatted_errs
                )
            }
            // 注意：这个错误纯粹是信息性的，不应该暴露给用户
            ErrorKind::LockAcquireError => write!(
                f,
                "无法获取 Volta 目录的锁"
            ),
            ErrorKind::NoBundledNpm { command } => write!(
                f,
                "无法检测到捆绑的 npm 版本。

请确保您已使用 `volta {} node` 选择了 Node 版本（更多信息请参见 `volta help {0}`）。",
                command
            ),
            ErrorKind::NoCommandLinePnpm => write!(
                f,
                "未指定 pnpm 版本。

使用 `volta run --pnpm` 选择一个版本（更多信息请参见 `volta help run`）。"
            ),
            ErrorKind::NoCommandLineYarn => write!(
                f,
                "未指定 Yarn 版本。

使用 `volta run --yarn` 选择一个版本（更多信息请参见 `volta help run`）。"
            ),
            ErrorKind::NoDefaultNodeVersion { tool } => write!(
                f,
                "无法安装 {} 因为未设置默认的 Node 版本。

首先使用 `volta install node` 选择默认的 Node，然后安装 {0} 版本。",
                                tool
            ),
            ErrorKind::NodeVersionNotFound { matching } => write!(
                f,
                r#"在版本注册表中找不到匹配 "{}" 的 Node 版本。

请验证版本是否正确。"#,
                matching
            ),
            ErrorKind::NoHomeEnvironmentVar => write!(
                f,
                "无法确定主目录。

请确保设置了环境变量 'HOME'。"
            ),
            ErrorKind::NoInstallDir => write!(
                f,
                "无法确定 Volta 安装目录。

请确保正确安装了 Volta"
            ),
            ErrorKind::NoLocalDataDir => write!(
                f,
                "无法确定 LocalAppData 目录。

请确保该目录可用。"
            ),
            ErrorKind::NoPinnedNodeVersion { tool } => write!(
                f,
                "无法固定 {} 因为此项目中未固定 Node 版本。

首先使用 `volta pin node` 固定 Node，然后固定 {0} 版本。",
                tool
            ),
            ErrorKind::NoPlatform => write!(
                f,
                "Node 不可用。

要运行任何 Node 命令，请先使用 `volta install node` 设置默认版本"
            ),
            ErrorKind::NoProjectNodeInManifest => write!(
                f,
                "在此项目中找不到 Node 版本。

使用 `volta pin node` 选择一个版本（更多信息请参见 `volta help pin`）。"
            ),
            ErrorKind::NoProjectPnpm => write!(
                f,
                "在此项目中找不到 pnpm 版本。

使用 `volta pin pnpm` 选择一个版本（更多信息请参见 `volta help pin`）。"
            ),
            ErrorKind::NoProjectYarn => write!(
                f,
                "在此项目中找不到 Yarn 版本。

使用 `volta pin yarn` 选择一个版本（更多信息请参见 `volta help pin`）。"
            ),
            ErrorKind::NoShellProfile { env_profile, bin_dir } => write!(
                f,
                "无法找到用户配置文件。
尝试了 $PROFILE ({})、~/.bashrc、~/.bash_profile、~/.zshenv ~/.zshrc、~/.profile 和 ~/.config/fish/config.fish

请创建其中之一并重试；或者您可以手动编辑您的配置文件以将 '{}' 添加到您的 PATH",
                env_profile, bin_dir.display()
            ),
            ErrorKind::NotInPackage => write!(
                f,
                "不在 node 包中。

使用 `volta install` 选择工具的默认版本。"
            ),
            ErrorKind::NoDefaultPnpm => write!(
                f,
                "pnpm 不可用。

使用 `volta install pnpm` 选择默认版本（更多信息请参见 `volta help install`）。"
            ),
            ErrorKind::NoDefaultYarn => write!(
                f,
                "Yarn 不可用。

使用 `volta install yarn` 选择默认版本（更多信息请参见 `volta help install`）。"
            ),
            ErrorKind::NpmLinkMissingPackage { package } => write!(
                f,
                "无法找到包 '{}'

请确保通过在其源目录中运行 `npm link` 使其可用。",
                package
            ),
            ErrorKind::NpmLinkWrongManager { package } => write!(
                f,
                "包 '{}' 不是使用 npm 安装的，无法使用 `npm link` 链接

请确保使用 `npm link` 链接它或使用 `npm i -g {0}` 安装它。",
                package
            ),
            ErrorKind::NpmVersionNotFound { matching } => write!(
                f,
                r#"在版本注册表中找不到匹配 "{}" 的 Node 版本。

请验证版本是否正确。"#,
                matching
            ),
            ErrorKind::NpxNotAvailable { version } => write!(
                f,
                "'npx' 仅在 npm >= 5.2.0 时可用

此项目配置为使用 npm 版本 {}。",
                version
            ),
            ErrorKind::PackageInstallFailed { package } => write!(
                f,
                "无法安装包 '{}'

请确认包是有效的，并使用 `--verbose` 运行以获取更多诊断信息。",
                package
            ),
            ErrorKind::PackageManifestParseError { package } => write!(
                f,
                "无法解析 {} 的 package.json 清单

请确保包包含有效的清单文件。",
                package
            ),
            ErrorKind::PackageManifestReadError { package } => write!(
                f,
                "无法读取 {} 的 package.json 清单

请确保包包含有效的清单文件。",
                package
            ),
            ErrorKind::PackageNotFound { package } => write!(
                f,
                "在包注册表中找不到 '{}'。

请验证请求的包是否正确。",
                package
            ),
            ErrorKind::PackageParseError { file } => write!(
                f,
                "无法解析项目清单
在 {}

请确保文件格式正确。",
                file.display()
            ),
            ErrorKind::PackageReadError { file } => write!(
                f,
                "无法读取项目清单
从 {}

请确保文件存在。",
                file.display()
            ),
            ErrorKind::PackageUnpackError => write!(
                f,
                "无法确定包目录布局。

请确保包格式正确。"
            ),
            ErrorKind::PackageWriteError { file } => write!(
                f,
                "无法写入项目清单
到 {}

请确保您有正确的权限。",
                file.display()
            ),
            ErrorKind::ParseBinConfigError => write!(
                f,
                "无法解析可执行文件配置文件。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::ParseHooksError { file } => write!(
                f,
                "无法解析钩子配置文件。
从 {}

请确保文件格式正确。",
                file.display()
            ),
            ErrorKind::ParseNodeIndexCacheError => write!(
                f,
                "无法解析 Node 索引缓存文件。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::ParseNodeIndexError { from_url } => write!(
                f,
                "无法解析 Node 版本索引
从 {}

请验证您的互联网连接。",
                from_url
            ),
            ErrorKind::ParseNodeIndexExpiryError => write!(
                f,
                "无法解析 Node 索引缓存过期文件。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::ParseNpmManifestError => write!(
                f,
                "无法解析捆绑 npm 的 package.json 文件。

请确保 Node 版本正确。"
            ),
            ErrorKind::ParsePackageConfigError => write!(
                f,
                "无法解析包配置文件。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::ParsePlatformError => write!(
                f,
                "无法解析平台设置文件。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::ParseToolSpecError { tool_spec } => write!(
                f,
                "无法解析工具规格 `{}`

请提供格式为 `<工具名称>[@<版本>]` 的规格。",
                tool_spec
            ),
            ErrorKind::PersistInventoryError { tool } => write!(
                f,
                "无法将 {} 存档存储在库存缓存中

{}",
                tool, PERMISSIONS_CTA
            ),
            ErrorKind::PnpmVersionNotFound { matching } => write!(
                f,
                r#"在版本注册表中找不到匹配 "{}" 的 pnpm 版本。

请验证版本是否正确。"#,
                matching
            ),
            ErrorKind::ProjectLocalBinaryExecError { command } => write!(
                f,
                "无法执行 `{}`

请确保您有正确的权限访问该文件。",
                command
            ),
            ErrorKind::ProjectLocalBinaryNotFound { command } => write!(
                f,
                "在您的项目中找不到可执行文件 `{}`。

请确保使用 `npm install` 或 `yarn install` 安装了所有项目依赖项",
                command
            ),
            ErrorKind::PublishHookBothUrlAndBin => write!(
                f,
                "发布钩子配置包含两种钩子类型。

请只包含 'bin' 或 'url' 中的一个"
            ),
            ErrorKind::PublishHookNeitherUrlNorBin => write!(
                f,
                "发布钩子配置不包含任何钩子类型。

请包含 'bin' 或 'url' 中的一个"
            ),
            ErrorKind::ReadBinConfigDirError { dir } => write!(
                f,
                "无法读取可执行文件元数据目录
在 {}

{}",
                dir.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::ReadBinConfigError { file } => write!(
                f,
                "无法读取可执行文件配置
从 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::ReadDefaultNpmError { file } => write!(
                f,
                "无法读取默认 npm 版本
从 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::ReadDirError { dir } => write!(
                f,
                "无法读取目录 {} 的内容

{}",
                dir.display(), PERMISSIONS_CTA
            ),
            ErrorKind::ReadHooksError { file } => write!(
                f,
                "无法读取钩子文件
从 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::ReadNodeIndexCacheError { file } => write!(
                f,
                "无法读取 Node 索引缓存
从 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::ReadNodeIndexExpiryError { file } => write!(
                f,
                "无法读取 Node 索引缓存过期时间
从 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::ReadNpmManifestError => write!(
                f,
                "无法读取捆绑 npm 的 package.json 文件。

请确保 Node 版本正确。"
            ),
            ErrorKind::ReadPackageConfigError { file } => write!(
                f,
                "无法读取包配置文件
从 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::ReadPlatformError { file } => write!(
                f,
                "无法读取默认平台文件
从 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            #[cfg(windows)]
            ErrorKind::ReadUserPathError => write!(
                f,
                "无法读取用户 Path 环境变量。

请确保您有权访问您的环境变量。"
            ),
            ErrorKind::RegistryFetchError { tool, from_url } => write!(
                f,
                "无法下载 {} 版本注册表
从 {}

请验证您的互联网连接。",
                tool, from_url
            ),
            ErrorKind::RunShimDirectly => write!(
                f,
                "'volta-shim' 不应直接调用。

请使用 Volta 提供的现有 shim（node、yarn 等）来运行工具。"
            ),
            ErrorKind::SetToolExecutable { tool } => write!(
                f,
                r#"无法将 "{}" 设置为可执行

{}"#,
                tool, PERMISSIONS_CTA
            ),
            ErrorKind::SetupToolImageError { tool, version, dir } => write!(
                f,
                "无法为 {} v{} 创建环境
在 {}

{}",
                tool,
                version,
                dir.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::ShimCreateError { name } => write!(
                f,
                r#"无法为 "{}" 创建 shim

{}"#,
                name, PERMISSIONS_CTA
            ),
            ErrorKind::ShimRemoveError { name } => write!(
                f,
                r#"无法移除 "{}" 的 shim

{}"#,
                name, PERMISSIONS_CTA
            ),
            ErrorKind::StringifyBinConfigError => write!(
                f,
                "无法序列化可执行文件配置。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::StringifyPackageConfigError => write!(
                f,
                "无法序列化包配置。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::StringifyPlatformError => write!(
                f,
                "无法序列化平台设置。

{}",
                REPORT_BUG_CTA
            ),
            ErrorKind::Unimplemented { feature } => {
                write!(f, "{}尚不支持。", feature)
            }
            ErrorKind::UnpackArchiveError { tool, version } => write!(
                f,
                "无法解压 {} v{}

请确保指定了正确的版本。",
                tool, version
            ),
            ErrorKind::UpgradePackageNotFound { package, manager } => write!(
                f,
                r#"无法找到要升级的包 '{}'。

请确保使用 `{} {0}` 安装它"#,
                package,
                match manager {
                    PackageManager::Npm => "npm i -g",
                    PackageManager::Pnpm => "pnpm add -g",
                    PackageManager::Yarn => "yarn global add",
                }
            ),
            ErrorKind::UpgradePackageWrongManager { package, manager } => {
                let (name, command) = match manager {
                    PackageManager::Npm => ("npm", "npm update -g"),
                    PackageManager::Pnpm => ("pnpm", "pnpm update -g"),
                    PackageManager::Yarn => ("Yarn", "yarn global upgrade"),
                };
                write!(
                    f,
                    r#"包 '{}' 是使用 {} 安装的。

要升级它，请使用命令 `{} {0}`"#,
                    package, name, command
                )
            }
            ErrorKind::VersionParseError { version } => write!(
                f,
                r#"无法解析版本 "{}"

请验证预期的版本。"#,
                version
            ),
            ErrorKind::WriteBinConfigError { file } => write!(
                f,
                "无法写入可执行文件配置
到 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::WriteDefaultNpmError { file } => write!(
                f,
                "无法写入捆绑的 npm 版本
到 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::WriteLauncherError { tool } => write!(
                f,
                "无法为 {} 设置启动器

这很可能是一个临时故障，请重试。",
                tool
            ),
            ErrorKind::WriteNodeIndexCacheError { file } => write!(
                f,
                "无法写入 Node 索引缓存
到 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::WriteNodeIndexExpiryError { file } => write!(
                f,
                "无法写入 Node 索引缓存过期时间
到 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::WritePackageConfigError { file } => write!(
                f,
                "无法写入包配置
到 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            ErrorKind::WritePlatformError { file } => write!(
                f,
                "无法保存平台设置
到 {}

{}",
                file.display(),
                PERMISSIONS_CTA
            ),
            #[cfg(windows)]
            ErrorKind::WriteUserPathError => write!(
                f,
                "无法写入 Path 环境变量。

请确保您有权编辑您的环境变量。"
            ),
            ErrorKind::Yarn2NotSupported => write!(
                f,
                "不建议使用 Yarn 2 版本，Volta 也不支持。

请改用 3 或更高版本。"
            ),
            ErrorKind::YarnLatestFetchError { from_url } => write!(
                f,
                "无法从 {} 获取 Yarn 的最新版本

请检查您的网络连接。",
                from_url
            ),
            ErrorKind::YarnVersionNotFound { matching } => write!(
                f,
                r#"在版本注册表中找不到匹配 "{}" 的 Yarn 版本。

请验证版本是否正确。"#,
                matching
            ),
    }
    }
}

impl ErrorKind {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            ErrorKind::BinaryAlreadyInstalled { .. } => ExitCode::FileSystemError,
            ErrorKind::BinaryExecError => ExitCode::ExecutionFailure,
            ErrorKind::BinaryNotFound { .. } => ExitCode::ExecutableNotFound,
            ErrorKind::BuildPathError => ExitCode::EnvironmentError,
            ErrorKind::BypassError { .. } => ExitCode::ExecutionFailure,
            ErrorKind::CannotFetchPackage { .. } => ExitCode::InvalidArguments,
            ErrorKind::CannotPinPackage { .. } => ExitCode::InvalidArguments,
            ErrorKind::CompletionsOutFileError { .. } => ExitCode::InvalidArguments,
            ErrorKind::ContainingDirError { .. } => ExitCode::FileSystemError,
            ErrorKind::CouldNotDetermineTool => ExitCode::UnknownError,
            ErrorKind::CouldNotStartMigration => ExitCode::EnvironmentError,
            ErrorKind::CreateDirError { .. } => ExitCode::FileSystemError,
            ErrorKind::CreateLayoutFileError { .. } => ExitCode::FileSystemError,
            ErrorKind::CreateSharedLinkError { .. } => ExitCode::FileSystemError,
            ErrorKind::CreateTempDirError { .. } => ExitCode::FileSystemError,
            ErrorKind::CreateTempFileError { .. } => ExitCode::FileSystemError,
            ErrorKind::CurrentDirError => ExitCode::EnvironmentError,
            ErrorKind::DeleteDirectoryError { .. } => ExitCode::FileSystemError,
            ErrorKind::DeleteFileError { .. } => ExitCode::FileSystemError,
            ErrorKind::DeprecatedCommandError { .. } => ExitCode::InvalidArguments,
            ErrorKind::DownloadToolNetworkError { .. } => ExitCode::NetworkError,
            ErrorKind::ExecuteHookError { .. } => ExitCode::ExecutionFailure,
            ErrorKind::ExtensionCycleError { .. } => ExitCode::ConfigurationError,
            ErrorKind::ExtensionPathError { .. } => ExitCode::FileSystemError,
            ErrorKind::HookCommandFailed { .. } => ExitCode::ConfigurationError,
            ErrorKind::HookMultipleFieldsSpecified => ExitCode::ConfigurationError,
            ErrorKind::HookNoFieldsSpecified => ExitCode::ConfigurationError,
            ErrorKind::HookPathError { .. } => ExitCode::ConfigurationError,
            ErrorKind::InstalledPackageNameError => ExitCode::UnknownError,
            ErrorKind::InvalidHookCommand { .. } => ExitCode::ExecutableNotFound,
            ErrorKind::InvalidHookOutput { .. } => ExitCode::ExecutionFailure,
            ErrorKind::InvalidInvocation { .. } => ExitCode::InvalidArguments,
            ErrorKind::InvalidInvocationOfBareVersion { .. } => ExitCode::InvalidArguments,
            ErrorKind::InvalidRegistryFormat { .. } => ExitCode::ConfigurationError,
            ErrorKind::InvalidToolName { .. } => ExitCode::InvalidArguments,
            ErrorKind::LockAcquireError => ExitCode::FileSystemError,
            ErrorKind::NoBundledNpm { .. } => ExitCode::ConfigurationError,
            ErrorKind::NoCommandLinePnpm => ExitCode::ConfigurationError,
            ErrorKind::NoCommandLineYarn => ExitCode::ConfigurationError,
            ErrorKind::NoDefaultNodeVersion { .. } => ExitCode::ConfigurationError,
            ErrorKind::NodeVersionNotFound { .. } => ExitCode::NoVersionMatch,
            ErrorKind::NoHomeEnvironmentVar => ExitCode::EnvironmentError,
            ErrorKind::NoInstallDir => ExitCode::EnvironmentError,
            ErrorKind::NoLocalDataDir => ExitCode::EnvironmentError,
            ErrorKind::NoPinnedNodeVersion { .. } => ExitCode::ConfigurationError,
            ErrorKind::NoPlatform => ExitCode::ConfigurationError,
            ErrorKind::NoProjectNodeInManifest => ExitCode::ConfigurationError,
            ErrorKind::NoProjectPnpm => ExitCode::ConfigurationError,
            ErrorKind::NoProjectYarn => ExitCode::ConfigurationError,
            ErrorKind::NoShellProfile { .. } => ExitCode::EnvironmentError,
            ErrorKind::NotInPackage => ExitCode::ConfigurationError,
            ErrorKind::NoDefaultPnpm => ExitCode::ConfigurationError,
            ErrorKind::NoDefaultYarn => ExitCode::ConfigurationError,
            ErrorKind::NpmLinkMissingPackage { .. } => ExitCode::ConfigurationError,
            ErrorKind::NpmLinkWrongManager { .. } => ExitCode::ConfigurationError,
            ErrorKind::NpmVersionNotFound { .. } => ExitCode::NoVersionMatch,
            ErrorKind::NpxNotAvailable { .. } => ExitCode::ExecutableNotFound,
            ErrorKind::PackageInstallFailed { .. } => ExitCode::UnknownError,
            ErrorKind::PackageManifestParseError { .. } => ExitCode::ConfigurationError,
            ErrorKind::PackageManifestReadError { .. } => ExitCode::FileSystemError,
            ErrorKind::PackageNotFound { .. } => ExitCode::InvalidArguments,
            ErrorKind::PackageParseError { .. } => ExitCode::ConfigurationError,
            ErrorKind::PackageReadError { .. } => ExitCode::FileSystemError,
            ErrorKind::PackageUnpackError => ExitCode::ConfigurationError,
            ErrorKind::PackageWriteError { .. } => ExitCode::FileSystemError,
            ErrorKind::ParseBinConfigError => ExitCode::UnknownError,
            ErrorKind::ParseHooksError { .. } => ExitCode::ConfigurationError,
            ErrorKind::ParseToolSpecError { .. } => ExitCode::InvalidArguments,
            ErrorKind::ParseNodeIndexCacheError => ExitCode::UnknownError,
            ErrorKind::ParseNodeIndexError { .. } => ExitCode::NetworkError,
            ErrorKind::ParseNodeIndexExpiryError => ExitCode::UnknownError,
            ErrorKind::ParseNpmManifestError => ExitCode::UnknownError,
            ErrorKind::ParsePackageConfigError => ExitCode::UnknownError,
            ErrorKind::ParsePlatformError => ExitCode::ConfigurationError,
            ErrorKind::PersistInventoryError { .. } => ExitCode::FileSystemError,
            ErrorKind::PnpmVersionNotFound { .. } => ExitCode::NoVersionMatch,
            ErrorKind::ProjectLocalBinaryExecError { .. } => ExitCode::ExecutionFailure,
            ErrorKind::ProjectLocalBinaryNotFound { .. } => ExitCode::FileSystemError,
            ErrorKind::PublishHookBothUrlAndBin => ExitCode::ConfigurationError,
            ErrorKind::PublishHookNeitherUrlNorBin => ExitCode::ConfigurationError,
            ErrorKind::ReadBinConfigDirError { .. } => ExitCode::FileSystemError,
            ErrorKind::ReadBinConfigError { .. } => ExitCode::FileSystemError,
            ErrorKind::ReadDefaultNpmError { .. } => ExitCode::FileSystemError,
            ErrorKind::ReadDirError { .. } => ExitCode::FileSystemError,
            ErrorKind::ReadHooksError { .. } => ExitCode::FileSystemError,
            ErrorKind::ReadNodeIndexCacheError { .. } => ExitCode::FileSystemError,
            ErrorKind::ReadNodeIndexExpiryError { .. } => ExitCode::FileSystemError,
            ErrorKind::ReadNpmManifestError => ExitCode::UnknownError,
            ErrorKind::ReadPackageConfigError { .. } => ExitCode::FileSystemError,
            ErrorKind::ReadPlatformError { .. } => ExitCode::FileSystemError,
            #[cfg(windows)]
            ErrorKind::ReadUserPathError => ExitCode::EnvironmentError,
            ErrorKind::RegistryFetchError { .. } => ExitCode::NetworkError,
            ErrorKind::RunShimDirectly => ExitCode::InvalidArguments,
            ErrorKind::SetupToolImageError { .. } => ExitCode::FileSystemError,
            ErrorKind::SetToolExecutable { .. } => ExitCode::FileSystemError,
            ErrorKind::ShimCreateError { .. } => ExitCode::FileSystemError,
            ErrorKind::ShimRemoveError { .. } => ExitCode::FileSystemError,
            ErrorKind::StringifyBinConfigError => ExitCode::UnknownError,
            ErrorKind::StringifyPackageConfigError => ExitCode::UnknownError,
            ErrorKind::StringifyPlatformError => ExitCode::UnknownError,
            ErrorKind::Unimplemented { .. } => ExitCode::UnknownError,
            ErrorKind::UnpackArchiveError { .. } => ExitCode::UnknownError,
            ErrorKind::UpgradePackageNotFound { .. } => ExitCode::ConfigurationError,
            ErrorKind::UpgradePackageWrongManager { .. } => ExitCode::ConfigurationError,
            ErrorKind::VersionParseError { .. } => ExitCode::NoVersionMatch,
            ErrorKind::WriteBinConfigError { .. } => ExitCode::FileSystemError,
            ErrorKind::WriteDefaultNpmError { .. } => ExitCode::FileSystemError,
            ErrorKind::WriteLauncherError { .. } => ExitCode::FileSystemError,
            ErrorKind::WriteNodeIndexCacheError { .. } => ExitCode::FileSystemError,
            ErrorKind::WriteNodeIndexExpiryError { .. } => ExitCode::FileSystemError,
            ErrorKind::WritePackageConfigError { .. } => ExitCode::FileSystemError,
            ErrorKind::WritePlatformError { .. } => ExitCode::FileSystemError,
            #[cfg(windows)]
            ErrorKind::WriteUserPathError => ExitCode::EnvironmentError,
            ErrorKind::Yarn2NotSupported => ExitCode::NoVersionMatch,
            ErrorKind::YarnLatestFetchError { .. } => ExitCode::NetworkError,
            ErrorKind::YarnVersionNotFound { .. } => ExitCode::NoVersionMatch,
        }
    }
}
