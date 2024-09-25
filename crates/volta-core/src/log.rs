//! 此模块为 `log` crate 提供了一个自定义的 Logger 实现
use console::style;
use log::{trace, Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::env;
use std::fmt::Display;
use std::io::IsTerminal;
use textwrap::{fill, Options, WordSplitter};

use crate::style::text_width;

const ERROR_PREFIX: &str = "error:";
const WARNING_PREFIX: &str = "warning:";
const SHIM_ERROR_PREFIX: &str = "Volta error:";
const SHIM_WARNING_PREFIX: &str = "Volta warning:";
const MIGRATION_ERROR_PREFIX: &str = "Volta update error:";
const MIGRATION_WARNING_PREFIX: &str = "Volta update warning:";
const VOLTA_LOGLEVEL: &str = "VOLTA_LOGLEVEL";
const ALLOWED_PREFIXES: [&str; 5] = [
    "volta",
    "archive",
    "fs-utils",
    "progress-read",
    "validate-npm-package-name",
];
const WRAP_INDENT: &str = "    ";

/// 表示创建日志记录器的上下文
pub enum LogContext {
    /// 来自 `volta` 可执行文件的日志消息
    Volta,

    /// 来自某个 shim 的日志消息
    Shim,

    /// 来自迁移的日志消息
    Migration,
}

/// 表示用户请求的详细程度级别
#[derive(Debug, Copy, Clone)]
pub enum LogVerbosity {
    Quiet,
    Default,
    Verbose,
    VeryVerbose,
}

pub struct Logger {
    context: LogContext,
    level: LevelFilter,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        let level_allowed = self.enabled(record.metadata());

        let is_valid_target = ALLOWED_PREFIXES
            .iter()
            .any(|prefix| record.target().starts_with(prefix));

        if level_allowed && is_valid_target {
            match record.level() {
                Level::Error => self.log_error(record.args()),
                Level::Warn => self.log_warning(record.args()),
                // 所有 info 级别的消息都发送到 stdout
                Level::Info => println!("{}", record.args()),
                // 所有 debug 和 trace 级别的消息都发送到 stderr
                Level::Debug => eprintln!("[verbose] {}", record.args()),
                Level::Trace => eprintln!("[trace] {}", record.args()),
            }
        }
    }

    fn flush(&self) {}
}

impl Logger {
    /// 使用 Logger 实例初始化全局日志记录器
    /// 将使用请求的详细程度级别
    /// 如果设置为 Default，将使用环境来确定详细程度级别
    pub fn init(context: LogContext, verbosity: LogVerbosity) -> Result<(), SetLoggerError> {
        let logger = Logger::new(context, verbosity);
        log::set_max_level(logger.level);
        log::set_boxed_logger(Box::new(logger))?;
        Ok(())
    }

    fn new(context: LogContext, verbosity: LogVerbosity) -> Self {
        let level = match verbosity {
            LogVerbosity::Quiet => LevelFilter::Error,
            LogVerbosity::Default => level_from_env(),
            LogVerbosity::Verbose => LevelFilter::Debug,
            LogVerbosity::VeryVerbose => LevelFilter::Trace,
        };

        Logger { context, level }
    }

    fn log_error<D>(&self, message: &D)
    where
        D: Display,
    {
        let prefix = match &self.context {
            LogContext::Volta => ERROR_PREFIX,
            LogContext::Shim => SHIM_ERROR_PREFIX,
            LogContext::Migration => MIGRATION_ERROR_PREFIX,
        };

        eprintln!("{} {}", style(prefix).red().bold(), message);
    }

    fn log_warning<D>(&self, message: &D)
    where
        D: Display,
    {
        let prefix = match &self.context {
            LogContext::Volta => WARNING_PREFIX,
            LogContext::Shim => SHIM_WARNING_PREFIX,
            LogContext::Migration => MIGRATION_WARNING_PREFIX,
        };

        eprintln!(
            "{} {}",
            style(prefix).yellow().bold(),
            wrap_content(prefix, message)
        );
    }
}

/// 如果我们在终端中，将提供的内容换行到终端宽度。
/// 如果不是，则将内容作为字符串返回
///
/// 注意：使用提供的前缀计算终端宽度，但随后将其删除，以便可以设置样式（样式字符计入换行宽度）
fn wrap_content<D>(prefix: &str, content: &D) -> String
where
    D: Display,
{
    match text_width() {
        Some(width) => {
            let options = Options::new(width)
                .word_splitter(WordSplitter::NoHyphenation)
                .subsequent_indent(WRAP_INDENT)
                .break_words(false);

            fill(&format!("{} {}", prefix, content), options).replace(prefix, "")
        }
        None => format!(" {}", content),
    }
}

/// 根据环境确定正确的日志级别
/// 如果 VOLTA_LOGLEVEL 设置为有效级别，我们使用它
/// 如果没有，我们检查当前的 stdout 以确定它是否是 TTY
///     如果是 TTY，我们使用 Info
///     如果不是 TTY，我们使用 Error，因为我们不想在作为脚本运行时显示警告
fn level_from_env() -> LevelFilter {
    env::var(VOLTA_LOGLEVEL)
        .ok()
        .and_then(|level| level.to_uppercase().parse().ok())
        .unwrap_or_else(|| {
            if std::io::stdout().is_terminal() {
                trace!("使用回退日志级别（info）");
                LevelFilter::Info
            } else {
                LevelFilter::Error
            }
        })
}

#[cfg(test)]
mod tests {}
