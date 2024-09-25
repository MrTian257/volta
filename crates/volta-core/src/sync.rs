//! Volta目录的进程间锁定
//!
//! 为了避免多个独立的Volta调用同时修改数据目录的问题，
//! 我们提供了一个锁定机制，一次只允许一个进程修改目录。
//!
//! 然而，在单个进程内，我们可能会在不同的代码路径中尝试锁定目录。
//! 例如，在安装包时我们需要一个锁，但我们也可能需要安装Node，
//! 这也需要一个锁。为了避免这些情况下的死锁，我们全局跟踪锁的状态：
//!
//! - 如果请求锁且没有活动锁，则我们在`volta.lock`文件上获取文件锁，
//!   并将状态初始化为计数1
//! - 如果锁已存在，则我们增加活动锁的计数
//! - 当不再需要锁时，我们减少活动锁的计数
//! - 当最后一个锁被释放时，我们释放文件锁并清除全局锁状态。
//!
//! 这允许多个代码路径请求锁而不用担心潜在的死锁，
//! 同时仍然防止多个进程进行并发更改。

use std::fs::{File, OpenOptions};
use std::marker::PhantomData;
use std::ops::Drop;
use std::sync::Mutex;

use crate::error::{Context, ErrorKind, Fallible};
use crate::layout::volta_home;
use crate::style::progress_spinner;
use fs2::FileExt;
use log::debug;
use once_cell::sync::Lazy;

// 全局锁状态
static LOCK_STATE: Lazy<Mutex<Option<LockState>>> = Lazy::new(|| Mutex::new(None));

/// 此进程的当前锁状态。
///
/// 注意：为确保此进程内的线程安全，我们将状态封装在Mutex中。
/// 这个Mutex及其相关锁与整体进程锁是分开的，
/// 仅用于确保在给定进程内准确维护计数。
struct LockState {
    file: File,
    count: usize,
}

const LOCK_FILE: &str = "volta.lock";

/// Volta目录进程锁的RAII实现。一个给定的Volta进程可以有
/// 多个活动锁，但一次只有一个进程可以有任何锁。
///
/// 一旦所有的`VoltaLock`对象超出作用域，锁将被释放给其他进程。
pub struct VoltaLock {
    // 私有字段确保这只能通过`acquire()`方法创建
    _private: PhantomData<()>,
}

impl VoltaLock {
    pub fn acquire() -> Fallible<Self> {
        let mut state = LOCK_STATE
            .lock()
            .with_context(|| ErrorKind::LockAcquireError)?;

        // 检查此进程是否有活动锁。如果有，增加活动锁的计数。
        // 如果没有，创建文件锁并将状态初始化为计数1
        match &mut *state {
            Some(inner) => {
                inner.count += 1;
            }
            None => {
                let path = volta_home()?.root().join(LOCK_FILE);
                debug!("正在获取Volta目录的锁: {}", path.display());

                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(path)
                    .with_context(|| ErrorKind::LockAcquireError)?;
                // 首先我们尝试不阻塞地锁定文件。如果失败，则显示一个spinner
                // 并阻塞直到锁定完成。
                if file.try_lock_exclusive().is_err() {
                    let spinner = progress_spinner("等待Volta目录的文件锁");
                    // 注意：阻塞直到文件可以被锁定
                    let lock_result = file
                        .lock_exclusive()
                        .with_context(|| ErrorKind::LockAcquireError);
                    spinner.finish_and_clear();
                    lock_result?;
                }

                *state = Some(LockState { file, count: 1 });
            }
        }

        Ok(Self {
            _private: PhantomData,
        })
    }
}

impl Drop for VoltaLock {
    fn drop(&mut self) {
        // 在drop时，减少活动锁的计数。如果计数为1，
        // 则这是最后一个活动锁，所以解锁文件并
        // 清除锁状态。
        if let Ok(mut state) = LOCK_STATE.lock() {
            match &mut *state {
                Some(inner) => {
                    if inner.count == 1 {
                        debug!("解锁Volta目录");
                        let _ = inner.file.unlock();
                        *state = None;
                    } else {
                        inner.count -= 1;
                    }
                }
                None => {
                    debug!("意外解锁未锁定的Volta目录");
                }
            }
        }
    }
}
