use std::ffi::OsStr;
use std::process::Command;

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(windows)] {
        pub fn create_command<E>(exe: E) -> Command
        where
            E: AsRef<OsStr>
        {
            // 许多 Node 工具是以 `.bat` 或 `.cmd` 文件的形式实现的
            // 当使用 `Command` 执行这些文件时，我们需要用以下方式调用它们：
            //    cmd.exe /C <命令> <参数>
            // 而不是: <命令> <参数>
            // 参见: https://github.com/rust-lang/rust/issues/42791 获取更详细的讨论
            let mut command = Command::new("cmd.exe");
            command.arg("/C");
            command.arg(exe);
            command
        }
    } else {
        pub fn create_command<E>(exe: E) -> Command
        where
            E: AsRef<OsStr>
        {
            // 在非 Windows 系统上，直接创建命令
            Command::new(exe)
        }
    }
}
