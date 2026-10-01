//! Lilyco 域模型切片 —— **A 组：当前 lilyco-core 的真实依赖形态**
//! serde(derive) + serde_json + thiserror（std）。与 B/C 组对外 API 完全一致，
//! 仅 JSON 后端与错误派生方式不同，用于量化「换依赖」的收益。

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

// ── Error（thiserror 派生，与 lilyco-core 同）─────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum AppError {
    #[error("参数错误: {0}")]
    InvalidArg(String),
    #[error("输入无效: {0}")]
    InvalidInput(String),
    #[error("执行失败: {0}")]
    Runtime(String),
    #[error("已取消")]
    Cancelled,
    #[error("安全门: {0}")]
    Safety(String),
    #[error("IO 错误: {0}")]
    Io(String),
    #[error("序列化错误: {0}")]
    Json(String),
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RegistryError {
    #[error("命令重复注册: {0}")]
    Duplicate(String),
    #[error("命令不存在: {0}")]
    NotFound(String),
}

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

// ── JSON 出口 ─────────────────────────────────────────────

fn ser<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
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
    serde_json::from_str(s).map_err(|_| AppError::Json(String::from("bad schema")))
}
















