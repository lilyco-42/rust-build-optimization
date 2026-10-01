//! Lilyco 域模型切片 —— 与 `lilyco-core` 同构（ArgSchema / ArgKind / CommandSchema /
//! Progress / LogLevel / AppError / Registry），用于 no_std 编译速度与体积对照实验。
//!
//! 本文件是 **B（no_std）与 C（std）两个变体共用的代码体**，逐字一致；
//! 两者唯一差别是文件头的 `#![no_std]`。

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

// ── Schema ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArgSchema {
    pub name: String,
    pub kind: ArgKind,
    pub required: bool,
    pub help: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArgKind {
    Str,
    Int,
    Float,
    Bool,
    Path,
    Enum { values: Vec<String> },
    List { item: Box<ArgKind> },
}

impl ArgKind {
    pub fn tag(&self) -> &'static str {
        match self {
            ArgKind::Str => "str",
            ArgKind::Int => "int",
            ArgKind::Float => "float",
            ArgKind::Bool => "bool",
            ArgKind::Path => "path",
            ArgKind::Enum { .. } => "enum",
            ArgKind::List { .. } => "list",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandSchema {
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub args: Vec<ArgSchema>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub danger: bool,
}

impl CommandSchema {
    pub fn new(name: &str, summary: &str) -> Self {
        CommandSchema {
            name: String::from(name),
            summary: String::from(summary),
            args: Vec::new(),
            tags: Vec::new(),
            danger: false,
        }
    }
    pub fn arg(mut self, s: ArgSchema) -> Self {
        self.args.push(s);
        self
    }
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueEnum {
    pub value: String,
    pub label: String,
}

// ── Progress ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Progress {
    Started {
        total: Option<u64>,
        message: Option<String>,
    },
    Tick {
        current: u64,
        total: Option<u64>,
        message: Option<String>,
        percent: Option<f32>,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    Telemetry {
        key: String,
        value: f64,
    },
    Done {
        result: String,
        duration_ms: u64,
    },
    Error {
        code: i32,
        message: String,
        kind: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

// ── Error ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppError {
    InvalidArg(String),
    InvalidInput(String),
    Runtime(String),
    Cancelled,
    Safety(String),
    Io(String),
    Json(String),
}

impl core::fmt::Display for AppError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AppError::InvalidArg(m) => write!(f, "参数错误: {m}"),
            AppError::InvalidInput(m) => write!(f, "输入无效: {m}"),
            AppError::Runtime(m) => write!(f, "执行失败: {m}"),
            AppError::Cancelled => f.write_str("已取消"),
            AppError::Safety(m) => write!(f, "安全门: {m}"),
            AppError::Io(m) => write!(f, "IO 错误: {m}"),
            AppError::Json(m) => write!(f, "序列化错误: {m}"),
        }
    }
}

impl core::error::Error for AppError {}

impl AppError {
    pub fn variant_name(&self) -> &'static str {
        match self {
            AppError::InvalidArg(_) => "invalid_arg",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::Runtime(_) => "runtime",
            AppError::Cancelled => "cancelled",
            AppError::Safety(_) => "safety",
            AppError::Io(_) => "io",
            AppError::Json(_) => "json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryError {
    Duplicate(String),
    NotFound(String),
}

impl core::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RegistryError::Duplicate(n) => write!(f, "命令重复注册: {n}"),
            RegistryError::NotFound(n) => write!(f, "命令不存在: {n}"),
        }
    }
}

impl core::error::Error for RegistryError {}

// ── Registry ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisteredCommand {
    pub name: String,
    pub schema: CommandSchema,
    pub handler_id: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    pub cmds: Vec<RegisteredCommand>,
}

impl Registry {
    pub fn new() -> Self {
        Registry { cmds: Vec::new() }
    }

    pub fn register(&mut self, name: &str, summary: &str) -> Result<u32, RegistryError> {
        if self.cmds.iter().any(|c| c.name.as_str() == name) {
            return Err(RegistryError::Duplicate(String::from(name)));
        }
        let id = self.cmds.len() as u32;
        self.cmds.push(RegisteredCommand {
            name: String::from(name),
            schema: CommandSchema::new(name, summary),
            handler_id: id,
        });
        Ok(id)
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredCommand> {
        self.cmds.iter().find(|c| c.name.as_str() == name)
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        self.cmds.iter().map(|c| c.name.clone()).collect()
    }
}

// ── JSON 出口（三端消费的统一形态）────────────────────────

const BUF: usize = 4096;

fn ser<T: Serialize>(v: &T) -> String {
    let mut buf = [0u8; BUF];
    match serde_json_core::ser::to_slice(v, &mut buf) {
        Ok(n) => String::from_utf8_lossy(&buf[..n]).into_owned(),
        Err(_) => String::new(),
    }
}

pub fn schema_to_json(c: &CommandSchema) -> String {
    ser(c)
}

pub fn progress_to_json(p: &Progress) -> String {
    ser(p)
}

pub fn error_to_json(e: &AppError) -> String {
    ser(e)
}

pub fn registry_to_json(r: &Registry) -> String {
    ser(r)
}

pub fn parse_command(s: &str) -> Result<CommandSchema, AppError> {
    match serde_json_core::de::from_str::<CommandSchema>(s) {
        Ok((v, _)) => Ok(v),
        Err(e) => Err(AppError::Json(alloc::format!("{:?}", e))),
    }
}
