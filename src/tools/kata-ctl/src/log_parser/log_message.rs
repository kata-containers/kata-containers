// Copyright (c) 2023 Gabe Venberg
//
// SPDX-License-Identifier: Apache-2.0

use std::{fmt::Debug, fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_with::{
    serde_as, skip_serializing_none, DeserializeFromStr, DisplayFromStr, SerializeDisplay,
};
use thiserror::Error;

pub trait AnyLogMessage: Serialize + DeserializeOwned + Debug {
    fn get_timestamp(&self) -> DateTime<Utc>;
}

#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct LogMessage {
    pub level: Option<LogLevel>,

    #[serde(rename = "msg")]
    pub message: String,

    pub name: Option<String>,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub pid: Option<usize>,

    pub source: Option<String>,

    pub subsystem: Option<String>,

    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "ts")]
    pub timestamp: DateTime<Utc>,
}

impl AnyLogMessage for LogMessage {
    fn get_timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

//totally abusing serde to easily display this.
impl Display for LogMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string_pretty(&self).map_err(|_| std::fmt::Error)?
        )
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct StrictLogMessage {
    pub level: LogLevel,

    #[serde(rename = "msg")]
    pub message: String,

    pub name: String,

    #[serde_as(as = "DisplayFromStr")]
    pub pid: usize,

    pub source: String,

    pub subsystem: String,

    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "ts")]
    pub timestamp: DateTime<Utc>,
}

impl AnyLogMessage for StrictLogMessage {
    fn get_timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

//totally abusing serde to easily display this.
impl Display for StrictLogMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string_pretty(&self).map_err(|_| std::fmt::Error)?
        )
    }
}

// A newtype for slog::Level, as it does not implement Serialize and Deserialize.
#[derive(Debug, SerializeDisplay, DeserializeFromStr, PartialEq, Eq)]
pub struct LogLevel(slog::Level);

#[derive(Debug, Error)]
pub enum LevelError {
    #[error("invalid slog level: {0}")]
    InvalidLevel(String),
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0.as_str()))
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel(slog::Level::Info)
    }
}

impl FromStr for LogLevel {
    type Err = LevelError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let level = match s.to_lowercase().as_str() {
            //need to accept both the short and long string versions.
            "critical" | "crit" => slog::Level::Critical,
            "error" | "erro" => slog::Level::Error,
            "warning" | "warn" => slog::Level::Warning,
            "info" => slog::Level::Info,
            "debug" | "debg" => slog::Level::Debug,
            "trace" | "trce" => slog::Level::Trace,
            _ => return Err(LevelError::InvalidLevel(s.to_string())),
        };

        Ok(LogLevel(level))
    }
}

//TODO: add tests for serialization.
#[cfg(test)]
mod test {
    use super::*;
    use crate::log_parser::log_parser_error::LogParserError;

    #[test]
    fn parse_string() {
        let log = r#"{"msg":"vmm-master thread is uninitialized or has exited.","level":"DEBG","ts":"2023-03-15T14:17:02.526992506Z","pid":"3327263","version":"0.1.0","name":"kata-runtime","subsystem":"hypervisor","source":"foo"}"#;
        let result = Ok(LogMessage {
            level: Some(LogLevel(slog::Level::Debug)),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: Some("kata-runtime".to_string()),
            pid: Some(3327263),
            source: Some("foo".to_string()),
            subsystem: Some("hypervisor".to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        });
        assert_eq!(
            serde_json::from_str(log)
                .map_err(|e| LogParserError::ParsingError(e, r#"Will not happen"#.to_string())),
            result
        )
    }

    #[test]
    fn parse_string_with_missing_fields() {
        let log = r#"{"msg":"vmm-master thread is uninitialized or has exited.","level":"DEBG","ts":"2023-03-15T14:17:02.526992506Z","version":"0.1.0","subsystem":"hypervisor","source":"foo"}"#;
        let result = Ok(LogMessage {
            level: Some(LogLevel(slog::Level::Debug)),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: None,
            pid: None,
            source: Some("foo".to_string()),
            subsystem: Some("hypervisor".to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        });
        assert_eq!(
            serde_json::from_str(log)
                .map_err(|e| LogParserError::ParsingError(e, r#"Will not happen"#.to_string())),
            result
        )
    }

    #[test]
    #[should_panic]
    fn parse_error() {
        let log = "random non-kata log message";
        serde_json::from_str::<LogMessage>(log).unwrap();
    }

    #[test]
    fn parse_string_strict() {
        let log = r#"{"msg":"vmm-master thread is uninitialized or has exited.","level":"DEBG","ts":"2023-03-15T14:17:02.526992506Z","pid":"3327263","version":"0.1.0","name":"kata-runtime","subsystem":"hypervisor","source":"foo"}"#;
        let result = Ok(StrictLogMessage {
            level: LogLevel(slog::Level::Debug),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: "kata-runtime".to_string(),
            pid: 3327263,
            source: "foo".to_string(),
            subsystem: "hypervisor".to_string(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        });
        assert_eq!(
            serde_json::from_str(log)
                .map_err(|e| LogParserError::ParsingError(e, r#"Will not happen"#.to_string())),
            result
        )
    }

    #[test]
    #[should_panic]
    fn parse_string_with_missing_fields_strict() {
        let log = r#"{"msg":"vmm-master thread is uninitialized or has exited.","level":"DEBG","ts":"2023-03-15T14:17:02.526992506Z","version":"0.1.0","subsystem":"hypervisor","source":"foo"}"#;
        println!(
            "{:?}",
            serde_json::from_str::<StrictLogMessage>(log).unwrap()
        );
    }

    #[test]
    #[should_panic]
    fn parse_error_strict() {
        let log = "random non-kata log message";
        serde_json::from_str::<StrictLogMessage>(log).unwrap();
    }

    #[test]
    fn serialize_json_strict() {
        let result = r#"{"level":"DEBUG","msg":"vmm-master thread is uninitialized or has exited.","name":"kata-runtime","pid":"3327263","source":"foo","subsystem":"hypervisor","ts":"2023-03-15 14:17:02.526992506 UTC"}"#;
        let log = StrictLogMessage {
            level: LogLevel(slog::Level::Debug),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: "kata-runtime".to_string(),
            pid: 3327263,
            source: "foo".to_string(),
            subsystem: "hypervisor".to_string(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, serde_json::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_json() {
        let result = r#"{"level":"DEBUG","msg":"vmm-master thread is uninitialized or has exited.","pid":"3327263","subsystem":"hypervisor","ts":"2023-03-15 14:17:02.526992506 UTC"}"#;
        let log = LogMessage {
            level: Some(LogLevel(slog::Level::Debug)),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: None,
            pid: Some(3327263),
            source: None,
            subsystem: Some("hypervisor".to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, serde_json::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_csv_strict() {
        let result = r#"level,msg,name,pid,source,subsystem,ts
DEBUG,vmm-master thread is uninitialized or has exited.,kata-runtime,3327263,foo,hypervisor,2023-03-15 14:17:02.526992506 UTC
"#;
        let log = StrictLogMessage {
            level: LogLevel(slog::Level::Debug),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: "kata-runtime".to_string(),
            pid: 3327263,
            source: "foo".to_string(),
            subsystem: "hypervisor".to_string(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        let mut csv_writer = csv::Writer::from_writer(vec![]);
        csv_writer.serialize(&log).unwrap();
        let output = String::from_utf8(csv_writer.into_inner().unwrap()).unwrap();
        assert_eq!(result, output);
    }

    #[test]
    fn serialize_csv() {
        let result = r#"level,msg,pid,subsystem,ts
DEBUG,vmm-master thread is uninitialized or has exited.,3327263,hypervisor,2023-03-15 14:17:02.526992506 UTC
"#;
        let log = LogMessage {
            level: Some(LogLevel(slog::Level::Debug)),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: None,
            pid: Some(3327263),
            source: None,
            subsystem: Some("hypervisor".to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        let mut csv_writer = csv::Writer::from_writer(vec![]);
        csv_writer.serialize(&log).unwrap();
        let output = String::from_utf8(csv_writer.into_inner().unwrap()).unwrap();
        assert_eq!(result, output);
    }

    #[test]
    fn serialize_ron_strict() {
        let result = r#"(level:"DEBUG",msg:"vmm-master thread is uninitialized or has exited.",name:"kata-runtime",pid:"3327263",source:"foo",subsystem:"hypervisor",ts:"2023-03-15 14:17:02.526992506 UTC")"#;
        let log = StrictLogMessage {
            level: LogLevel(slog::Level::Debug),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: "kata-runtime".to_string(),
            pid: 3327263,
            source: "foo".to_string(),
            subsystem: "hypervisor".to_string(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, ron::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_ron() {
        let result = r#"(level:Some("DEBUG"),msg:"vmm-master thread is uninitialized or has exited.",pid:Some("3327263"),subsystem:Some("hypervisor"),ts:"2023-03-15 14:17:02.526992506 UTC")"#;
        let log = LogMessage {
            level: Some(LogLevel(slog::Level::Debug)),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: None,
            pid: Some(3327263),
            source: None,
            subsystem: Some("hypervisor".to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, ron::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_toml_strict() {
        let result = r#"level = "DEBUG"
msg = "vmm-master thread is uninitialized or has exited."
name = "kata-runtime"
pid = "3327263"
source = "foo"
subsystem = "hypervisor"
ts = "2023-03-15 14:17:02.526992506 UTC"
"#;
        let log = StrictLogMessage {
            level: LogLevel(slog::Level::Debug),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: "kata-runtime".to_string(),
            pid: 3327263,
            source: "foo".to_string(),
            subsystem: "hypervisor".to_string(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, toml::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_toml() {
        let result = r#"level = "DEBUG"
msg = "vmm-master thread is uninitialized or has exited."
pid = "3327263"
subsystem = "hypervisor"
ts = "2023-03-15 14:17:02.526992506 UTC"
"#;
        let log = LogMessage {
            level: Some(LogLevel(slog::Level::Debug)),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: None,
            pid: Some(3327263),
            source: None,
            subsystem: Some("hypervisor".to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, toml::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_xml_strict() {
        let result = r#"<StrictLogMessage><level>DEBUG</level><msg>vmm-master thread is uninitialized or has exited.</msg><name>kata-runtime</name><pid>3327263</pid><source>foo</source><subsystem>hypervisor</subsystem><ts>2023-03-15 14:17:02.526992506 UTC</ts></StrictLogMessage>"#;
        let log = StrictLogMessage {
            level: LogLevel(slog::Level::Debug),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: "kata-runtime".to_string(),
            pid: 3327263,
            source: "foo".to_string(),
            subsystem: "hypervisor".to_string(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, quick_xml::se::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_xml() {
        let result = r#"<LogMessage><level>DEBUG</level><msg>vmm-master thread is uninitialized or has exited.</msg><pid>3327263</pid><subsystem>hypervisor</subsystem><ts>2023-03-15 14:17:02.526992506 UTC</ts></LogMessage>"#;
        let log = LogMessage {
            level: Some(LogLevel(slog::Level::Debug)),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: None,
            pid: Some(3327263),
            source: None,
            subsystem: Some("hypervisor".to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, quick_xml::se::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_yaml_strict() {
        let result = r#"level: DEBUG
msg: vmm-master thread is uninitialized or has exited.
name: kata-runtime
pid: '3327263'
source: foo
subsystem: hypervisor
ts: 2023-03-15 14:17:02.526992506 UTC
"#;
        let log = StrictLogMessage {
            level: LogLevel(slog::Level::Debug),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: "kata-runtime".to_string(),
            pid: 3327263,
            source: "foo".to_string(),
            subsystem: "hypervisor".to_string(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, serde_yaml::to_string(&log).unwrap());
    }

    #[test]
    fn serialize_yaml() {
        let result = r#"level: DEBUG
msg: vmm-master thread is uninitialized or has exited.
pid: '3327263'
subsystem: hypervisor
ts: 2023-03-15 14:17:02.526992506 UTC
"#;
        let log = LogMessage {
            level: Some(LogLevel(slog::Level::Debug)),
            message: "vmm-master thread is uninitialized or has exited.".to_string(),
            name: None,
            pid: Some(3327263),
            source: None,
            subsystem: Some("hypervisor".to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2023-03-15T14:17:02.526992506Z")
                .unwrap()
                .into(),
        };
        assert_eq!(result, serde_yaml::to_string(&log).unwrap());
    }
}
