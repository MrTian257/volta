//! 提供用于在 Unix 操作系统中获取和解压 Node 安装 tarball 的类型和函数。

use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::{content_length, Archive, ArchiveError, Origin};
use flate2::read::GzDecoder;
use fs_utils::ensure_containing_dir_exists;
use progress_read::ProgressRead;
use tee::TeeReader;

/// Node 安装 tarball。
pub struct Tarball {
    compressed_size: u64,
    data: Box<dyn Read>,
    origin: Origin,
}

impl Tarball {
    /// 从指定文件加载 tarball。
    pub fn load(source: File) -> Result<Box<dyn Archive>, ArchiveError> {
        let compressed_size = source.metadata()?.len();
        Ok(Box::new(Tarball {
            compressed_size,
            data: Box::new(source),
            origin: Origin::Local,
        }))
    }

    /// 从给定 URL 开始获取 tarball，返回一个可以流式传输的 tarball
    /// （并且在流式传输时将其数据复制到本地文件）。
    pub fn fetch(url: &str, cache_file: &Path) -> Result<Box<dyn Archive>, ArchiveError> {
        let (status, headers, response) = attohttpc::get(url).send()?.split();

        if !status.is_success() {
            return Err(ArchiveError::HttpError(status));
        }

        let compressed_size = content_length(&headers)?;

        ensure_containing_dir_exists(&cache_file)?;
        let file = File::create(cache_file)?;
        let data = Box::new(TeeReader::new(response, file));

        Ok(Box::new(Tarball {
            compressed_size,
            data,
            origin: Origin::Remote,
        }))
    }
}

impl Archive for Tarball {
    /// 返回压缩后的大小
    fn compressed_size(&self) -> u64 {
        self.compressed_size
    }
    /// 解压 tarball 到指定目录
    fn unpack(
        self: Box<Self>,
        dest: &Path,
        progress: &mut dyn FnMut(&(), usize),
    ) -> Result<(), ArchiveError> {
        let decoded = GzDecoder::new(ProgressRead::new(self.data, (), progress));
        let mut tarball = tar::Archive::new(decoded);
        tarball.unpack(dest)?;
        Ok(())
    }
    /// 返回 tarball 的来源
    fn origin(&self) -> Origin {
        self.origin
    }
}

#[cfg(test)]
pub mod tests {

    use crate::tarball::Tarball;
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
        let mut test_file_path = fixture_path("tarballs");
        test_file_path.push("test-file.tar.gz");
        let test_file = File::open(test_file_path).expect("无法打开测试文件");
        let tarball = Tarball::load(test_file).expect("加载 tarball 失败");

        assert_eq!(tarball.compressed_size(), 402);
    }
}
