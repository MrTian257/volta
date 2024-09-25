//! 提供用于操作文件系统的实用工具。

use std::fs::{self, create_dir_all, read_dir, DirEntry, File, Metadata};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::error::{Context, ErrorKind, Fallible};
use crate::layout::volta_home;
use retry::delay::Fibonacci;
use retry::{retry, OperationResult};
use tempfile::{tempdir_in, NamedTempFile, TempDir};

/// 打开一个文件，如果不存在则创建它
pub fn touch(path: &Path) -> io::Result<File> {
    if !path.is_file() {
        if let Some(basedir) = path.parent() {
            create_dir_all(basedir)?;
        }
        File::create(path)?;
    }
    File::open(path)
}

/// 如果目标目录存在，则删除它。如果目录不存在，则视为成功。
pub fn remove_dir_if_exists<P: AsRef<Path>>(path: P) -> Fallible<()> {
    fs::remove_dir_all(&path)
        .or_else(ok_if_not_found)
        .with_context(|| ErrorKind::DeleteDirectoryError {
            directory: path.as_ref().to_owned(),
        })
}

/// 如果目标文件存在，则删除它。如果文件不存在，则视为成功。
pub fn remove_file_if_exists<P: AsRef<Path>>(path: P) -> Fallible<()> {
    fs::remove_file(&path)
        .or_else(ok_if_not_found)
        .with_context(|| ErrorKind::DeleteFileError {
            file: path.as_ref().to_owned(),
        })
}

/// 将因文件未找到而导致的失败转换为成功。
///
/// 处理错误比在删除之前检查文件是否存在更可取，因为这避免了检查和删除之间的潜在竞争条件。
pub fn ok_if_not_found<T: Default>(err: io::Error) -> io::Result<T> {
    match err.kind() {
        io::ErrorKind::NotFound => Ok(T::default()),
        _ => Err(err),
    }
}

/// 如果文件存在，则读取它。
pub fn read_file<P: AsRef<Path>>(path: P) -> io::Result<Option<String>> {
    let result: io::Result<String> = fs::read_to_string(path);

    match result {
        Ok(string) => Ok(Some(string)),
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(error),
        },
    }
}

/// 读取目录的全部内容，急切地提取每个目录条目及其元数据，并返回它们的迭代器。
/// 如果这些步骤中的任何一个失败，则返回 `Error`。
///
/// 这个函数使得编写用于操作目录内容的高级逻辑（映射、过滤等）变得更容易。
///
/// 注意，这个函数会分配一个中间向量来存储目录条目，以便构造迭代器，
/// 所以如果预期目录非常大，它将分配与条目数量成比例的临时数据。
pub fn read_dir_eager(dir: &Path) -> io::Result<impl Iterator<Item = (DirEntry, Metadata)>> {
    let entries = read_dir(dir)?;
    let vec = entries
        .map(|entry| {
            let entry = entry?;
            let metadata = entry.metadata()?;
            Ok((entry, metadata))
        })
        .collect::<io::Result<Vec<(DirEntry, Metadata)>>>()?;

    Ok(vec.into_iter())
}

/// 读取目录的内容并返回输入函数匹配结果的 Vec
pub fn dir_entry_match<T, F>(dir: &Path, mut f: F) -> io::Result<Vec<T>>
where
    F: FnMut(&DirEntry) -> Option<T>,
{
    let entries = read_dir_eager(dir)?;
    Ok(entries
        .filter(|(_, metadata)| metadata.is_file())
        .filter_map(|(entry, _)| f(&entry))
        .collect::<Vec<T>>())
}

/// 在 Volta tmp 目录中创建一个 NamedTempFile
pub fn create_staging_file() -> Fallible<NamedTempFile> {
    let tmp_dir = volta_home()?.tmp_dir();
    NamedTempFile::new_in(tmp_dir).with_context(|| ErrorKind::CreateTempFileError {
        in_dir: tmp_dir.to_owned(),
    })
}

/// 在 Volta tmp 目录中创建一个临时目录
pub fn create_staging_dir() -> Fallible<TempDir> {
    let tmp_root = volta_home()?.tmp_dir();
    tempdir_in(tmp_root).with_context(|| ErrorKind::CreateTempDirError {
        in_dir: tmp_root.to_owned(),
    })
}

/// 创建文件符号链接。`dst` 路径将是一个指向 `src` 路径的符号链接。
pub fn symlink_file<S, D>(src: S, dest: D) -> io::Result<()>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(src, dest);

    #[cfg(unix)]
    return std::os::unix::fs::symlink(src, dest);
}

/// 创建目录符号链接。`dst` 路径将是一个指向 `src` 路径的符号链接。
pub fn symlink_dir<S, D>(src: S, dest: D) -> io::Result<()>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    #[cfg(windows)]
    return junction::create(src, dest);

    #[cfg(unix)]
    return std::os::unix::fs::symlink(src, dest);
}

/// 确保给定文件具有"可执行"权限，否则我们将无法调用它
#[cfg(unix)]
pub fn set_executable(bin: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(bin)?.permissions();
    let mode = permissions.mode();

    if mode & 0o111 != 0o111 {
        permissions.set_mode(mode | 0o111);
        fs::set_permissions(bin, permissions)
    } else {
        Ok(())
    }
}

/// 确保给定文件具有"可执行"权限，否则我们将无法调用它
///
/// 注意：这在 Windows 上是一个空操作，因为 Windows 没有"可执行"权限的概念
#[cfg(windows)]
pub fn set_executable(_bin: &Path) -> io::Result<()> {
    Ok(())
}

/// 将文件或目录重命名为新名称，如果操作因权限问题失败则重试
///
/// 将重试约30秒，每次重试之间的延迟越来越长，以允许病毒扫描和其他自动化操作完成。
pub fn rename<F, T>(from: F, to: T) -> io::Result<()>
where
    F: AsRef<Path>,
    T: AsRef<Path>,
{
    // 从1毫秒开始的21个斐波那契步骤总共约28秒
    // 参见 https://github.com/rust-lang/rustup/pull/1873，Rustup 使用这种方法来解决病毒扫描文件锁定问题
    let from = from.as_ref();
    let to = to.as_ref();

    retry(Fibonacci::from_millis(1).take(21), || {
        match fs::rename(from, to) {
            Ok(_) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::PermissionDenied => OperationResult::Retry(e),
                _ => OperationResult::Err(e),
            },
        }
    })
    .map_err(|e| e.error)
}
