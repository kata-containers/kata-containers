// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//
use rustjail::errors::*;
use std::fs;

const DEBUG_CONSOLE_FLAG: &'static str = "agent.debug_console";
const DEV_MODE_FLAG: &'static str = "agent.devmode";
const LOG_LEVEL_FLAG: &'static str = "agent.log";

const DEFAULT_LOG_LEVEL: slog::Level = slog::Level::Info;

// FIXME: unused
const TRACE_MODE_FLAG: &'static str = "agent.trace";
const USE_VSOCK_FLAG: &'static str = "agent.use_vsock";

#[derive(Debug)]
pub struct agentConfig {
    pub debug_console: bool,
    pub dev_mode: bool,
    pub log_level: slog::Level,
}

impl agentConfig {
    pub fn new() -> agentConfig {
        agentConfig {
            debug_console: false,
            dev_mode: false,
            log_level: DEFAULT_LOG_LEVEL,
        }
    }

    pub fn parse_cmdline(&mut self, file: &str) -> Result<()> {
        let cmdline = fs::read_to_string(file)?;
        let params: Vec<&str> = cmdline.split_ascii_whitespace().collect();
        for param in params.iter() {
            if param.starts_with(DEBUG_CONSOLE_FLAG) {
                self.debug_console = true;
            }

            if param.starts_with(DEV_MODE_FLAG) {
                self.dev_mode = true;
            }

            if param.starts_with(LOG_LEVEL_FLAG) {
                let level = get_log_level(param)?;
                self.log_level = level;
            }
        }

        Ok(())
    }
}

// Map logrus (https://godoc.org/github.com/sirupsen/logrus)
// log level to the equivalent slog log levels.
//
// Note: Logrus names are used for compatability with the previous
// golang-based agent.
fn logrus_to_slog_level(logrus_level: &str) -> Result<slog::Level> {
    let level = match logrus_level {
        // Note: different semantics to logrus: log, but don't panic.
        "fatal" | "panic" => slog::Level::Critical,

        "critical" => slog::Level::Critical,
        "error" => slog::Level::Error,
        "warn" | "warning" => slog::Level::Warning,
        "info" => slog::Level::Info,
        "debug" => slog::Level::Debug,

        // Not in logrus
        "trace" => slog::Level::Trace,

        _ => {
            return Err(ErrorKind::ErrorCode(String::from("invalid log level")).into());
        }
    };

    Ok(level)
}

fn get_log_level(param: &str) -> Result<slog::Level> {
    let fields: Vec<&str> = param.split("=").collect();

    if fields.len() != 2 {
        return Err(ErrorKind::ErrorCode(String::from("invalid log level parameter")).into());
    }

    let key = fields[0];

    if key != LOG_LEVEL_FLAG {
        return Err(ErrorKind::ErrorCode(String::from("invalid log level key name").into()).into());
    }

    let value = fields[1];

    let level = logrus_to_slog_level(value)?;

    Ok(level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    const ERR_INVALID_LOG_LEVEL: &str = "invalid log level";
    const ERR_INVALID_LOG_LEVEL_PARAM: &str = "invalid log level parameter";
    const ERR_INVALID_LOG_LEVEL_KEY: &str = "invalid log level key name";

    // helper function to make errors less crazy-long
    fn make_err(desc: &str) -> Error {
        ErrorKind::ErrorCode(desc.to_string()).into()
    }

    // Parameters:
    //
    // 1: expected Result
    // 2: actual Result
    // 3: string used to identify the test on error
    macro_rules! assert_log_level {
        ($expected_result:expr, $actual_result:expr, $msg:expr) => {
            if $expected_result.is_ok() {
                let expected_level = $expected_result.as_ref().unwrap();
                let actual_level = $actual_result.unwrap();
                assert!(*expected_level == actual_level, $msg);
            } else {
                let expected_error = $expected_result.as_ref().unwrap_err();
                let actual_error = $actual_result.unwrap_err();

                let expected_error_msg = format!("{:?}", expected_error);
                let actual_error_msg = format!("{:?}", actual_error);

                assert!(expected_error_msg == actual_error_msg, $msg);
            }
        };
    }

    #[test]
    fn test_new() {
        let config = agentConfig::new();
        assert_eq!(config.debug_console, false);
        assert_eq!(config.dev_mode, false);
        assert_eq!(config.log_level, DEFAULT_LOG_LEVEL);
    }

    #[test]
    fn test_parse_cmdline() {
        #[derive(Debug)]
        struct TestData<'a> {
            contents: &'a str,
            debug_console: bool,
            dev_mode: bool,
        }

        let tests = &[
            TestData {
                contents: "",
                debug_console: false,
                dev_mode: false,
            },
            TestData {
                contents: "foo",
                debug_console: false,
                dev_mode: false,
            },
            TestData {
                contents: "foo bar",
                debug_console: false,
                dev_mode: false,
            },
            TestData {
                contents: "foo bar",
                debug_console: false,
                dev_mode: false,
            },
            TestData {
                contents: "foo agent bar",
                debug_console: false,
                dev_mode: false,
            },
            TestData {
                contents: "foo debug_console agent bar devmode",
                debug_console: false,
                dev_mode: false,
            },
            TestData {
                contents: "agent.debug_console",
                debug_console: true,
                dev_mode: false,
            },
            TestData {
                contents: "   agent.debug_console ",
                debug_console: true,
                dev_mode: false,
            },
            TestData {
                contents: "agent.debug_console foo",
                debug_console: true,
                dev_mode: false,
            },
            TestData {
                contents: " agent.debug_console foo",
                debug_console: true,
                dev_mode: false,
            },
            TestData {
                contents: "foo agent.debug_console bar",
                debug_console: true,
                dev_mode: false,
            },
            TestData {
                contents: "foo agent.debug_console",
                debug_console: true,
                dev_mode: false,
            },
            TestData {
                contents: "foo agent.debug_console ",
                debug_console: true,
                dev_mode: false,
            },
            TestData {
                contents: "agent.devmode",
                debug_console: false,
                dev_mode: true,
            },
            TestData {
                contents: "   agent.devmode ",
                debug_console: false,
                dev_mode: true,
            },
            TestData {
                contents: "agent.devmode foo",
                debug_console: false,
                dev_mode: true,
            },
            TestData {
                contents: " agent.devmode foo",
                debug_console: false,
                dev_mode: true,
            },
            TestData {
                contents: "foo agent.devmode bar",
                debug_console: false,
                dev_mode: true,
            },
            TestData {
                contents: "foo agent.devmode",
                debug_console: false,
                dev_mode: true,
            },
            TestData {
                contents: "foo agent.devmode ",
                debug_console: false,
                dev_mode: true,
            },
            TestData {
                contents: "agent.devmode agent.debug_console",
                debug_console: true,
                dev_mode: true,
            },
        ];

        let dir = tempdir().expect("failed to create tmpdir");

        // First, check a missing file is handled
        let file_path = dir.path().join("enoent");

        let filename = file_path.to_str().expect("failed to create filename");

        let mut config = agentConfig::new();
        let result = config.parse_cmdline(&filename.to_owned());
        assert!(result.is_err());

        // Now, test various combinations of file contents
        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let file_path = dir.path().join("cmdline");

            let filename = file_path.to_str().expect("failed to create filename");

            let mut file =
                File::create(filename).expect(&format!("{}: failed to create file", msg));

            file.write_all(d.contents.as_bytes())
                .expect(&format!("{}: failed to write file contents", msg));

            let mut config = agentConfig::new();
            assert!(config.debug_console == false, msg);
            assert!(config.dev_mode == false, msg);

            let result = config.parse_cmdline(filename);
            assert!(result.is_ok(), "{}", msg);

            assert_eq!(d.debug_console, config.debug_console, "{}", msg);
            assert_eq!(d.dev_mode, config.dev_mode, "{}", msg);
        }
    }

    #[test]
    fn test_logrus_to_slog_level() {
        #[derive(Debug)]
        struct TestData<'a> {
            logrus_level: &'a str,
            result: Result<slog::Level>,
        }

        let tests = &[
            TestData {
                logrus_level: "",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL)),
            },
            TestData {
                logrus_level: "foo",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL)),
            },
            TestData {
                logrus_level: "debugging",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL)),
            },
            TestData {
                logrus_level: "xdebug",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL)),
            },
            TestData {
                logrus_level: "trace",
                result: Ok(slog::Level::Trace),
            },
            TestData {
                logrus_level: "debug",
                result: Ok(slog::Level::Debug),
            },
            TestData {
                logrus_level: "info",
                result: Ok(slog::Level::Info),
            },
            TestData {
                logrus_level: "warn",
                result: Ok(slog::Level::Warning),
            },
            TestData {
                logrus_level: "warning",
                result: Ok(slog::Level::Warning),
            },
            TestData {
                logrus_level: "error",
                result: Ok(slog::Level::Error),
            },
            TestData {
                logrus_level: "critical",
                result: Ok(slog::Level::Critical),
            },
            TestData {
                logrus_level: "fatal",
                result: Ok(slog::Level::Critical),
            },
            TestData {
                logrus_level: "panic",
                result: Ok(slog::Level::Critical),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = logrus_to_slog_level(d.logrus_level);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_log_level!(d.result, result, format!("{}", msg));
        }
    }

    #[test]
    fn test_get_log_level() {
        #[derive(Debug)]
        struct TestData<'a> {
            param: &'a str,
            result: Result<slog::Level>,
        }

        let tests = &[
            TestData {
                param: "",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_PARAM)),
            },
            TestData {
                param: "=",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_KEY)),
            },
            TestData {
                param: "x=",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_KEY)),
            },
            TestData {
                param: "=y",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_KEY)),
            },
            TestData {
                param: "==",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_PARAM)),
            },
            TestData {
                param: "= =",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_PARAM)),
            },
            TestData {
                param: "x=y",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_KEY)),
            },
            TestData {
                param: "agent=debug",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_KEY)),
            },
            TestData {
                param: "agent.logg=debug",
                result: Err(make_err(ERR_INVALID_LOG_LEVEL_KEY)),
            },
            TestData {
                param: "agent.log=trace",
                result: Ok(slog::Level::Trace),
            },
            TestData {
                param: "agent.log=debug",
                result: Ok(slog::Level::Debug),
            },
            TestData {
                param: "agent.log=info",
                result: Ok(slog::Level::Info),
            },
            TestData {
                param: "agent.log=warn",
                result: Ok(slog::Level::Warning),
            },
            TestData {
                param: "agent.log=warning",
                result: Ok(slog::Level::Warning),
            },
            TestData {
                param: "agent.log=error",
                result: Ok(slog::Level::Error),
            },
            TestData {
                param: "agent.log=critical",
                result: Ok(slog::Level::Critical),
            },
            TestData {
                param: "agent.log=fatal",
                result: Ok(slog::Level::Critical),
            },
            TestData {
                param: "agent.log=panic",
                result: Ok(slog::Level::Critical),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = get_log_level(d.param);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_log_level!(d.result, result, format!("{}", msg));
        }
    }
}
