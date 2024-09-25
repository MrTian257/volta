use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Stdio};

use log::debug;
use tempfile::NamedTempFile;

use crate::command::create_command;
use crate::event::Event;

/// 向生成的命令进程发送事件
// 如果未配置钩子命令，则不会调用此函数
pub fn send_events(command: &str, events: &[Event]) {
    match serde_json::to_string_pretty(&events) {
        Ok(events_json) => {
            // 如果设置了VOLTA_WRITE_EVENTS_FILE环境变量，则将事件写入临时文件
            let tempfile_path = env::var_os("VOLTA_WRITE_EVENTS_FILE")
                .and_then(|_| write_events_file(events_json.clone()));
            if let Some(ref mut child_process) = spawn_process(command, tempfile_path) {
                if let Some(ref mut p_stdin) = child_process.stdin.as_mut() {
                    // 将事件JSON写入子进程的标准输入
                    if let Err(error) = writeln!(p_stdin, "{}", events_json) {
                        debug!("无法将事件写入可执行文件的标准输入: {:?}", error);
                    }
                }
            }
        }
        Err(error) => {
            debug!("无法将事件数据序列化为JSON: {:?}", error);
        }
    }
}

// 将事件JSON写入临时目录中的文件
fn write_events_file(events_json: String) -> Option<PathBuf> {
    match NamedTempFile::new() {
        Ok(mut events_file) => {
            match events_file.write_all(events_json.as_bytes()) {
                Ok(()) => {
                    let path = events_file.into_temp_path();
                    // 如果不保留，临时文件将自动删除（可执行文件将无法读取）
                    match path.keep() {
                        Ok(tempfile_path) => Some(tempfile_path),
                        Err(error) => {
                            debug!("无法保留事件数据的临时文件: {:?}", error);
                            None
                        }
                    }
                }
                Err(error) => {
                    debug!("无法将事件写入临时文件: {:?}", error);
                    None
                }
            }
        }
        Err(error) => {
            debug!("无法为事件数据创建临时文件: {:?}", error);
            None
        }
    }
}

// 生成一个子进程来接收事件数据，将事件文件的路径设置为环境变量
fn spawn_process(command: &str, tempfile_path: Option<PathBuf>) -> Option<Child> {
    command.split(' ').take(1).next().and_then(|executable| {
        let mut child = create_command(executable);
        child.args(command.split(' ').skip(1));
        child.stdin(Stdio::piped());
        if let Some(events_file) = tempfile_path {
            child.env("EVENTS_FILE", events_file);
        }

        #[cfg(not(debug_assertions))]
        // 在发布模式下隐藏生成进程的stdout和stderr
        child.stdout(Stdio::null()).stderr(Stdio::null());

        match child.spawn() {
            Err(err) => {
                debug!("无法运行可执行命令: '{}'\n{}", command, err);
                None
            }
            Ok(c) => Some(c),
        }
    })
}
