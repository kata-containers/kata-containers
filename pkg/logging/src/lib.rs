// Copyright (c) 2019-2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use slog::{o, record_static, BorrowedKV, Drain, Key, OwnedKV, OwnedKVList, Record, KV};
use std::collections::HashMap;
use std::io;
use std::io::Write;
use std::process;
use std::result;
use std::sync::Mutex;

const LOG_LEVELS: &[(&str, slog::Level)] = &[
    ("trace", slog::Level::Trace),
    ("debug", slog::Level::Debug),
    ("info", slog::Level::Info),
    ("warn", slog::Level::Warning),
    ("error", slog::Level::Error),
    ("critical", slog::Level::Critical),
];

// XXX: 'writer' param used to make testing possible.
pub fn create_logger<W>(
    name: &str,
    source: &str,
    level: slog::Level,
    writer: W,
) -> (slog::Logger, slog_async::AsyncGuard)
where
    W: Write + Send + Sync + 'static,
{
    let json_drain = slog_json::Json::new(writer)
        .add_default_keys()
        .build()
        .fuse();

    // Ensure only a unique set of key/value fields is logged
    let unique_drain = UniqueDrain::new(json_drain).fuse();

    // Allow runtime filtering of records by log level
    let filter_drain = RuntimeLevelFilter::new(unique_drain, level).fuse();

    // Ensure the logger is thread-safe
    let (async_drain, guard) = slog_async::Async::new(filter_drain)
        .thread_name("slog-async-logger".into())
        .build_with_guard();

    // Add some "standard" fields
    let logger = slog::Logger::root(
        async_drain.fuse(),
        o!("version" => env!("CARGO_PKG_VERSION"),
            "subsystem" => "root",
            "pid" => process::id().to_string(),
            "name" => name.to_string(),
            "source" => source.to_string()),
    );

    (logger, guard)
}

pub fn get_log_levels() -> Vec<&'static str> {
    let result: Vec<&str> = LOG_LEVELS.iter().map(|value| value.0).collect();

    result
}

pub fn level_name_to_slog_level(level_name: &str) -> Result<slog::Level, String> {
    for tuple in LOG_LEVELS {
        if tuple.0 == level_name {
            return Ok(tuple.1);
        }
    }

    Err("invalid level name".to_string())
}

pub fn slog_level_to_level_name(level: slog::Level) -> Result<&'static str, &'static str> {
    for tuple in LOG_LEVELS {
        if tuple.1 == level {
            return Ok(tuple.0);
        }
    }

    Err("invalid slog level")
}

// Used to convert an slog::OwnedKVList into a hash map.
#[derive(Debug)]
struct HashSerializer {
    fields: HashMap<String, String>,
}

impl HashSerializer {
    fn new() -> HashSerializer {
        HashSerializer {
            fields: HashMap::new(),
        }
    }

    fn add_field(&mut self, key: String, value: String) {
        // Take care to only add the first instance of a key. This matters for loggers (but not
        // Records) since a child loggers have parents and the loggers are serialised child first
        // meaning the *newest* fields are serialised first.
        self.fields.entry(key).or_insert(value);
    }

    fn remove_field(&mut self, key: &str) {
        self.fields.remove(key);
    }
}

impl KV for HashSerializer {
    fn serialize(&self, _record: &Record, serializer: &mut dyn slog::Serializer) -> slog::Result {
        for (key, value) in self.fields.iter() {
            serializer.emit_str(Key::from(key.to_string()), value)?;
        }

        Ok(())
    }
}

impl slog::Serializer for HashSerializer {
    fn emit_arguments(&mut self, key: Key, value: &std::fmt::Arguments) -> slog::Result {
        self.add_field(format!("{}", key), format!("{}", value));
        Ok(())
    }
}

struct UniqueDrain<D> {
    drain: D,
}

impl<D> UniqueDrain<D> {
    fn new(drain: D) -> Self {
        UniqueDrain { drain }
    }
}

impl<D> Drain for UniqueDrain<D>
where
    D: Drain,
{
    type Ok = ();
    type Err = io::Error;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        let mut logger_serializer = HashSerializer::new();
        values.serialize(record, &mut logger_serializer)?;

        let mut record_serializer = HashSerializer::new();
        record.kv().serialize(record, &mut record_serializer)?;

        for (key, _) in record_serializer.fields.iter() {
            logger_serializer.remove_field(key);
        }

        let record_owned_kv = OwnedKV(record_serializer);
        let record_static = record_static!(record.level(), "");
        let new_record = Record::new(&record_static, record.msg(), BorrowedKV(&record_owned_kv));

        let logger_owned_kv = OwnedKV(logger_serializer);

        let result = self
            .drain
            .log(&new_record, &OwnedKVList::from(logger_owned_kv));

        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "failed to drain log".to_string(),
            )),
        }
    }
}

// A RuntimeLevelFilter will discard all log records whose log level is less than the level
// specified in the struct.
struct RuntimeLevelFilter<D> {
    drain: D,
    level: Mutex<slog::Level>,
}

impl<D> RuntimeLevelFilter<D> {
    fn new(drain: D, level: slog::Level) -> Self {
        RuntimeLevelFilter {
            drain,
            level: Mutex::new(level),
        }
    }
}

impl<D> Drain for RuntimeLevelFilter<D>
where
    D: Drain,
{
    type Ok = Option<D::Ok>;
    type Err = Option<D::Err>;

    fn log(
        &self,
        record: &slog::Record,
        values: &slog::OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        let log_level = self.level.lock().unwrap();

        if record.level().is_at_least(*log_level) {
            self.drain.log(record, values)?;
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use slog::info;
    use std::io::prelude::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_log_levels() {
        let expected = vec!["trace", "debug", "info", "warn", "error", "critical"];

        let log_levels = get_log_levels();
        assert_eq!(log_levels, expected);
    }

    #[test]
    fn test_level_name_to_slog_level() {
        #[derive(Debug)]
        struct TestData<'a> {
            name: &'a str,
            result: Result<slog::Level, &'a str>,
        }

        let invalid_msg = "invalid level name";

        let tests = &[
            TestData {
                name: "",
                result: Err(invalid_msg),
            },
            TestData {
                name: "foo",
                result: Err(invalid_msg),
            },
            TestData {
                name: "x",
                result: Err(invalid_msg),
            },
            TestData {
                name: ".",
                result: Err(invalid_msg),
            },
            TestData {
                name: "trace",
                result: Ok(slog::Level::Trace),
            },
            TestData {
                name: "debug",
                result: Ok(slog::Level::Debug),
            },
            TestData {
                name: "info",
                result: Ok(slog::Level::Info),
            },
            TestData {
                name: "warn",
                result: Ok(slog::Level::Warning),
            },
            TestData {
                name: "error",
                result: Ok(slog::Level::Error),
            },
            TestData {
                name: "critical",
                result: Ok(slog::Level::Critical),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = level_name_to_slog_level(d.name);

            let msg = format!("{}, result: {:?}", msg, result);

            if d.result.is_ok() {
                assert!(result.is_ok());

                let result_level = result.unwrap();
                let expected_level = d.result.unwrap();

                assert!(result_level == expected_level, msg);
                continue;
            } else {
                assert!(result.is_err(), msg);
            }

            let expected_error = format!("{}", d.result.as_ref().unwrap_err());
            let actual_error = format!("{}", result.unwrap_err());
            assert!(actual_error == expected_error, msg);
        }
    }

    #[test]
    fn test_slog_level_to_level_name() {
        #[derive(Debug)]
        struct TestData<'a> {
            level: slog::Level,
            result: Result<&'a str, &'a str>,
        }

        let tests = &[
            TestData {
                level: slog::Level::Trace,
                result: Ok("trace"),
            },
            TestData {
                level: slog::Level::Debug,
                result: Ok("debug"),
            },
            TestData {
                level: slog::Level::Info,
                result: Ok("info"),
            },
            TestData {
                level: slog::Level::Warning,
                result: Ok("warn"),
            },
            TestData {
                level: slog::Level::Error,
                result: Ok("error"),
            },
            TestData {
                level: slog::Level::Critical,
                result: Ok("critical"),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = slog_level_to_level_name(d.level);

            let msg = format!("{}, result: {:?}", msg, result);

            if d.result.is_ok() {
                assert!(result == d.result, msg);
                continue;
            }

            let expected_error = format!("{}", d.result.as_ref().unwrap_err());
            let actual_error = format!("{}", result.unwrap_err());
            assert!(actual_error == expected_error, msg);
        }
    }

    #[test]
    fn test_create_logger_write_to_tmpfile() {
        // Create a writer for the logger drain to use
        let writer = NamedTempFile::new().expect("failed to create tempfile");

        // Used to check file contents before the temp file is unlinked
        let mut writer_ref = writer.reopen().expect("failed to clone tempfile");

        let level = slog::Level::Trace;
        let name = "name";
        let source = "source";
        let record_subsystem = "record-subsystem";

        let record_key = "record-key-1";
        let record_value = "record-key-2";

        let logger = create_logger(name, source, level, writer);

        let msg = "foo, bar, baz";

        // Call the logger (which calls the drain)
        info!(logger, "{}", msg; "subsystem" => record_subsystem, record_key => record_value);

        // Force temp file to be flushed
        drop(logger);

        let mut contents = String::new();
        writer_ref
            .read_to_string(&mut contents)
            .expect("failed to read tempfile contents");

        // Convert file to JSON
        let fields: Value =
            serde_json::from_str(&contents).expect("failed to convert logfile to json");

        // Check the expected JSON fields

        let field_ts = fields.get("ts").expect("failed to find timestamp field");
        assert_ne!(field_ts, "");

        let field_version = fields.get("version").expect("failed to find version field");
        assert_eq!(field_version, env!("CARGO_PKG_VERSION"));

        let field_pid = fields.get("pid").expect("failed to find pid field");
        assert_ne!(field_pid, "");

        let field_level = fields.get("level").expect("failed to find level field");
        assert_eq!(field_level, "INFO");

        let field_msg = fields.get("msg").expect("failed to find msg field");
        assert_eq!(field_msg, msg);

        let field_name = fields.get("name").expect("failed to find name field");
        assert_eq!(field_name, name);

        let field_source = fields.get("source").expect("failed to find source field");
        assert_eq!(field_source, source);

        let field_subsystem = fields
            .get("subsystem")
            .expect("failed to find subsystem field");

        // The records field should take priority over the loggers field of the same name
        assert_eq!(field_subsystem, record_subsystem);

        let field_record_value = fields
            .get(record_key)
            .expect("failed to find record key field");
        assert_eq!(field_record_value, record_value);
    }
}
