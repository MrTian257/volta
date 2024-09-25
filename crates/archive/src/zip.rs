//! 提供在 Windows 操作系统中获取和解压 Node 安装 zip 文件的类型和函数。

use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::{content_length, ArchiveError};
use fs_utils::ensure_containing_dir_exists;
use progress_read::ProgressRead;
use tee::TeeReader;
use verbatim::PathExt;
use zip_rs::unstable::stream::ZipStreamReader;

use super::Archive;
use super::Origin;

/// Node 安装 zip 文件。
pub struct Zip {
    compressed_size: u64,
    data: Box<dyn Read>,
    origin: Origin,
}

impl Zip {
    /// 从指定文件加载缓存的 Node zip 归档。
    pub fn load(source: File) -> Result<Box<dyn Archive>, ArchiveError> {
        let compressed_size = source.metadata()?.len();

        Ok(Box::new(Zip {
            compressed_size,
            data: Box::new(source),
            origin: Origin::Local,
        }))
    }

    /// 从给定 URL 开始获取 Node zip 归档，返回一个 `Remote` 数据源。
    pub fn fetch(url: &str, cache_file: &Path) -> Result<Box<dyn Archive>, ArchiveError> {
        let (status, headers, response) = attohttpc::get(url).send()?.split();

        if !status.is_success() {
            return Err(ArchiveError::HttpError(status));
        }

        let compressed_size = content_length(&headers)?;

        ensure_containing_dir_exists(&cache_file)?;
        let file = File::create(cache_file)?;
        let data = Box::new(TeeReader::new(response, file));

        Ok(Box::new(Zip {
            compressed_size,
            data,
            origin: Origin::Remote,
        }))
    }
}

impl Archive for Zip {
    /// 返回压缩后的大小
    fn compressed_size(&self) -> u64 {
        self.compressed_size
    }
    /// 解压 zip 文件到指定目录
    fn unpack(
        self: Box<Self>,
        dest: &Path,
        progress: &mut dyn FnMut(&(), usize),
    ) -> Result<(), ArchiveError> {
        // 使用 verbatim 路径以避免 Windows 旧版 260 字节路径限制。
        let dest: &Path = &dest.to_verbatim();
        let zip = ZipStreamReader::new(ProgressRead::new(self.data, (), progress));
        zip.extract(dest)?;
        Ok(())
    }
    /// 返回 zip 文件的来源
    fn origin(&self) -> Origin {
        self.origin
    }
}

#[cfg(test)]
pub mod tests {

    use crate::zip::Zip;
    use std::fs::File;
    use std::path::PathBuf;

    /// 获取测试文件的路径
    fn fixture_path(fixture_dir: &str) -> PathBuf {
        let mut cargo_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        cargo_manifest_dir.push("fixtures");
        cargo_manifest_dir.push(fixture_dir);
        cargo_manifest_dir
    }

    #[test]
    fn test_load() {
        let mut test_file_path = fixture_path("zips");
        test_file_path.push("test-file.zip");
        let test_file = File::open(test_file_path).expect("无法打开测试文件");
        let zip = Zip::load(test_file).expect("加载 zip 文件失败");

        assert_eq!(zip.compressed_size(), 214);
    }
}
