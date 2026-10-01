//! E 组：与 D 组同样的 no_std + serde_json(alloc)，但 **serde 不开 derive**，
//! 手写 Serialize impl（用 serde 自己的 trait，不自己造序列化库）。
//! 目的：量化 serde_derive + syn + proc-macro2 这条串行关键路径值多少。

use alloc::string::String;
use alloc::vec::Vec;
use serde::ser::{Serialize, SerializeMap, SerializeStruct, Serializer};

#[derive(Debug, Clone, PartialEq)]
pub struct ArgSchema {
    pub name: String,
    pub kind: ArgKind,
    pub required: bool,
    pub help: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgKind {
    Str,
    Int,
    Float,
    Bool,
    Path,
    Enum { values: Vec<String> },
    List { item: alloc::boxed::Box<ArgKind> },
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

impl Serialize for ArgKind {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(1))?;
        m.serialize_entry("type", self.tag())?;
        m.end()
    }
}

impl Serialize for ArgSchema {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("ArgSchema", 4)?;
        st.serialize_field("name", &self.name)?;
        st.serialize_field("kind", &self.kind)?;
        st.serialize_field("required", &self.required)?;
        st.serialize_field("help", &self.help)?;
        if let Some(d) = &self.default {
            st.serialize_field("default", d)?;
        }
        st.end()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandSchema {
    pub name: String,
    pub summary: String,
    pub args: Vec<ArgSchema>,
    pub tags: Vec<String>,
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

impl Serialize for CommandSchema {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("CommandSchema", 5)?;
        st.serialize_field("name", &self.name)?;
        st.serialize_field("summary", &self.summary)?;
        st.serialize_field("args", &self.args)?;
        st.serialize_field("tags", &self.tags)?;
        st.serialize_field("danger", &self.danger)?;
        st.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl Serialize for LogLevel {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
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

impl Serialize for Progress {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        match self {
            Progress::Started { total, message } => {
                m.serialize_entry("type", "started")?;
                m.serialize_entry("total", total)?;
                m.serialize_entry("message", message)?;
            }
            Progress::Tick {
                current,
                total,
                message,
                percent,
            } => {
                m.serialize_entry("type", "tick")?;
                m.serialize_entry("current", current)?;
                m.serialize_entry("total", total)?;
                m.serialize_entry("message", message)?;
                m.serialize_entry("percent", percent)?;
            }
            Progress::Log { level, message } => {
                m.serialize_entry("type", "log")?;
                m.serialize_entry("level", level)?;
                m.serialize_entry("message", message)?;
            }
            Progress::Telemetry { key, value } => {
                m.serialize_entry("type", "telemetry")?;
                m.serialize_entry("key", key)?;
                m.serialize_entry("value", value)?;
            }
            Progress::Done {
                result,
                duration_ms,
            } => {
                m.serialize_entry("type", "done")?;
                m.serialize_entry("result", result)?;
                m.serialize_entry("duration_ms", duration_ms)?;
            }
            Progress::Error {
                code,
                message,
                kind,
            } => {
                m.serialize_entry("type", "error")?;
                m.serialize_entry("code", code)?;
                m.serialize_entry("message", message)?;
                m.serialize_entry("kind", kind)?;
            }
        }
        m.end()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    InvalidArg(String),
    InvalidInput(String),
    Runtime(String),
    Cancelled,
    Safety(String),
    Io(String),
    Json(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(1))?;
        match self {
            AppError::InvalidArg(v) => m.serialize_entry("invalid_arg", v)?,
            AppError::InvalidInput(v) => m.serialize_entry("invalid_input", v)?,
            AppError::Runtime(v) => m.serialize_entry("runtime", v)?,
            AppError::Cancelled => m.serialize_entry("cancelled", &())?,
            AppError::Safety(v) => m.serialize_entry("safety", v)?,
            AppError::Io(v) => m.serialize_entry("io", v)?,
            AppError::Json(v) => m.serialize_entry("json", v)?,
        }
        m.end()
    }
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredCommand {
    pub name: String,
    pub schema: CommandSchema,
    pub handler_id: u32,
}

#[derive(Debug, Clone, Default)]
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
    let names: Vec<&str> = r.cmds.iter().map(|c| c.name.as_str()).collect();
    ser(&names)
}
