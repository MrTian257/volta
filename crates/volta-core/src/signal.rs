use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};

use log::debug;

// 静态原子布尔值，用于指示垫片是否已获得控制权
static SHIM_HAS_CONTROL: AtomicBool = AtomicBool::new(false);
// 中断退出码常量
const INTERRUPTED_EXIT_CODE: i32 = 130;

// 将控制权传递给垫片
pub fn pass_control_to_shim() {
    // 将SHIM_HAS_CONTROL设置为true，使用SeqCst内存顺序
    SHIM_HAS_CONTROL.store(true, Ordering::SeqCst);
}

// 设置信号处理程序
pub fn setup_signal_handler() {
    // 设置Ctrl+C处理程序
    let result = ctrlc::set_handler(|| {
        // 如果垫片没有控制权，则退出程序
        if !SHIM_HAS_CONTROL.load(Ordering::SeqCst) {
            exit(INTERRUPTED_EXIT_CODE);
        }
    });

    // 如果无法设置处理程序，则记录调试信息
    if result.is_err() {
        debug!("无法设置Ctrl+C处理程序，SIGINT将无法正确处理");
    }
}
