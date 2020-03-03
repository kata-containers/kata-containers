// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//
use rustjail::errors::*;
use std::fs;
use std::time;

const DEBUG_CONSOLE_FLAG: &str = "agent.debug_console";
const DEV_MODE_FLAG: &str = "agent.devmode";
const LOG_LEVEL_OPTION: &str = "agent.log";
const HOTPLUG_TIMOUT_OPTION: &str = "agent.hotplug_timeout";
const DEBUG_CONSOLE_VPORT_OPTION: &str = "agent.debug_console_vport";
const LOG_VPORT_OPTION: &str = "agent.log_vport";
const CONTAINER_PIPE_SIZE_OPTION: &str = "agent.container_pipe_size";

const DEFAULT_LOG_LEVEL: slog::Level = slog::Level::Info;
const DEFAULT_HOTPLUG_TIMEOUT: time::Duration = time::Duration::from_secs(3);
const DEFAULT_CONTAINER_PIPE_SIZE: i32 = 0;

// FIXME: unused
const TRACE_MODE_FLAG: &str = "agent.trace";
const USE_VSOCK_FLAG: &str = "agent.use_vsock";

#[derive(Debug)]
pub struct agentConfig {
    pub debug_console: bool,
    pub dev_mode: bool,
    pub log_level: slog::Level,
    pub hotplug_timeout: time::Duration,
    pub debug_console_vport: i32,
    pub log_vport: i32,
    pub container_pipe_size: i32,
}

impl agentConfig {
    pub fn new() -> agentConfig {
        agentConfig {
            debug_console: false,
            dev_mode: false,
            log_level: DEFAULT_LOG_LEVEL,
            hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
            debug_console_vport: 0,
            log_vport: 0,
            container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
        }
    }

    pub fn parse_cmdline(&mut self, file: &str) -> Result<()> {
        let cmdline = fs::read_to_string(file)?;
        let params: Vec<&str> = cmdline.split_ascii_whitespace().collect();
        for param in params.iter() {
            if param.eq(&DEBUG_CONSOLE_FLAG) {
                self.debug_console = true;
            }

            if param.eq(&DEV_MODE_FLAG) {
                self.dev_mode = true;
            }

            if param.starts_with(format!("{}=", LOG_LEVEL_OPTION).as_str()) {
                let level = get_log_level(param)?;
                self.log_level = level;
            }

            if param.starts_with(format!("{}=", HOTPLUG_TIMOUT_OPTION).as_str()) {
                let hotplugTimeout = get_hotplug_timeout(param)?;
                // ensure the timeout is a positive value
                if hotplugTimeout.as_secs() > 0 {
                    self.hotplug_timeout = hotplugTimeout;
                }
            }

            if param.starts_with(format!("{}=", DEBUG_CONSOLE_VPORT_OPTION).as_str()) {
                let port = get_vsock_port(param)?;
                if port > 0 {
                    self.debug_console_vport = port;
                }
            }

            if param.starts_with(format!("{}=", LOG_VPORT_OPTION).as_str()) {
                let port = get_vsock_port(param)?;
                if port > 0 {
                    self.log_vport = port;
                }
            }

            if param.starts_with(format!("{}=", CONTAINER_PIPE_SIZE_OPTION).as_str()) {
                let container_pipe_size = get_container_pipe_size(param)?;
                self.container_pipe_size = container_pipe_size
            }
        }

        Ok(())
    }
}

fn get_vsock_port(p: &str) -> Result<i32> {
    let fields: Vec<&str> = p.split("=").collect();
    if fields.len() != 2 {
        return Err(ErrorKind::ErrorCode("invalid port parameter".to_string()).into());
    }

    Ok(fields[1].parse::<i32>()?)
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

    if fields[0] != LOG_LEVEL_OPTION {
        Err(ErrorKind::ErrorCode(String::from("invalid log level key name")).into())
    } else {
        Ok(logrus_to_slog_level(fields[1])?)
    }
}

fn get_hotplug_timeout(param: &str) -> Result<time::Duration> {
    let fields: Vec<&str> = param.split("=").collect();

    if fields.len() != 2 {
        return Err(ErrorKind::ErrorCode(String::from("invalid hotplug timeout parameter")).into());
    }

    let key = fields[0];
    if key != HOTPLUG_TIMOUT_OPTION {
        return Err(ErrorKind::ErrorCode(String::from("invalid hotplug timeout key name")).into());
    }

    let value = fields[1].parse::<u64>();
    if value.is_err() {
        return Err(ErrorKind::ErrorCode(String::from("unable to parse hotplug timeout")).into());
    }

    Ok(time::Duration::from_secs(value.unwrap()))
}

fn get_container_pipe_size(param: &str) -> Result<i32> {
    let fields: Vec<&str> = param.split("=").collect();

    if fields.len() != 2 {
        return Err(
            ErrorKind::ErrorCode(String::from("invalid container pipe size parameter")).into(),
        );
    }

    let key = fields[0];
    if key != CONTAINER_PIPE_SIZE_OPTION {
        return Err(
            ErrorKind::ErrorCode(String::from("invalid container pipe size key name")).into(),
        );
    }

    let res = fields[1].parse::<i32>();
    if res.is_err() {
        return Err(
            ErrorKind::ErrorCode(String::from("unable to parse container pipe size")).into(),
        );
    }

    let value = res.unwrap();
    if value < 0 {
        return Err(ErrorKind::ErrorCode(String::from(
            "container pipe size should not be negative",
        ))
        .into());
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::time;
    use tempfile::tempdir;

    const ERR_INVALID_LOG_LEVEL: &str = "invalid log level";
    const ERR_INVALID_LOG_LEVEL_PARAM: &str = "invalid log level parameter";
    const ERR_INVALID_LOG_LEVEL_KEY: &str = "invalid log level key name";

    const ERR_INVALID_HOTPLUG_TIMEOUT: &str = "invalid hotplug timeout parameter";
    const ERR_INVALID_HOTPLUG_TIMEOUT_PARAM: &str = "unable to parse hotplug timeout";
    const ERR_INVALID_HOTPLUG_TIMEOUT_KEY: &str = "invalid hotplug timeout key name";

    const ERR_INVALID_CONTAINER_PIPE_SIZE: &str = "invalid container pipe size parameter";
    const ERR_INVALID_CONTAINER_PIPE_SIZE_PARAM: &str = "unable to parse container pipe size";
    const ERR_INVALID_CONTAINER_PIPE_SIZE_KEY: &str = "invalid container pipe size key name";
    const ERR_INVALID_CONTAINER_PIPE_NEGATIVE: &str = "container pipe size should not be negative";

    // helper function to make errors less crazy-long
    fn make_err(desc: &str) -> Error {
        ErrorKind::ErrorCode(desc.to_string()).into()
    }

    // Parameters:
    //
    // 1: expected Result
    // 2: actual Result
    // 3: string used to identify the test on error
    macro_rules! assert_result {
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
        assert_eq!(config.hotplug_timeout, DEFAULT_HOTPLUG_TIMEOUT);
    }

    #[test]
    fn test_parse_cmdline() {
        #[derive(Debug)]
        struct TestData<'a> {
            contents: &'a str,
            debug_console: bool,
            dev_mode: bool,
            log_level: slog::Level,
            hotplug_timeout: time::Duration,
            container_pipe_size: i32,
        }

        let tests = &[
            TestData {
                contents: "agent.debug_consolex agent.devmode",
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.debug_console agent.devmodex",
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.logx=debug",
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.log=debug",
                debug_console: false,
                dev_mode: false,
                log_level: slog::Level::Debug,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "",
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo",
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo bar",
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo bar",
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo agent bar",
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo debug_console agent bar devmode",
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.debug_console",
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "   agent.debug_console ",
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.debug_console foo",
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: " agent.debug_console foo",
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo agent.debug_console bar",
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo agent.debug_console",
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo agent.debug_console ",
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.devmode",
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "   agent.devmode ",
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.devmode foo",
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: " agent.devmode foo",
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo agent.devmode bar",
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo agent.devmode",
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "foo agent.devmode ",
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.devmode agent.debug_console",
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.hotplug_timeout=100",
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: time::Duration::from_secs(100),
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.hotplug_timeout=0",
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.container_pipe_size=2097152",
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: 2097152,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.container_pipe_size=100",
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: 100,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.container_pipe_size=0",
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.container_pip_siz=100",
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
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
            assert_eq!(config.debug_console, false, "{}", msg);
            assert_eq!(config.dev_mode, false, "{}", msg);
            assert_eq!(
                config.hotplug_timeout,
                time::Duration::from_secs(3),
                "{}",
                msg
            );
            assert_eq!(config.container_pipe_size, 0, "{}", msg);

            let result = config.parse_cmdline(filename);
            assert!(result.is_ok(), "{}", msg);

            assert_eq!(d.debug_console, config.debug_console, "{}", msg);
            assert_eq!(d.dev_mode, config.dev_mode, "{}", msg);
            assert_eq!(d.log_level, config.log_level, "{}", msg);
            assert_eq!(d.hotplug_timeout, config.hotplug_timeout, "{}", msg);
            assert_eq!(d.container_pipe_size, config.container_pipe_size, "{}", msg);
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

            assert_result!(d.result, result, format!("{}", msg));
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

            assert_result!(d.result, result, format!("{}", msg));
        }
    }

    #[test]
    fn test_get_hotplug_timeout() {
        #[derive(Debug)]
        struct TestData<'a> {
            param: &'a str,
            result: Result<time::Duration>,
        }

        let tests = &[
            TestData {
                param: "",
                result: Err(make_err(ERR_INVALID_HOTPLUG_TIMEOUT)),
            },
            TestData {
                param: "agent.hotplug_timeout",
                result: Err(make_err(ERR_INVALID_HOTPLUG_TIMEOUT)),
            },
            TestData {
                param: "foo=bar",
                result: Err(make_err(ERR_INVALID_HOTPLUG_TIMEOUT_KEY)),
            },
            TestData {
                param: "agent.hotplug_timeot=1",
                result: Err(make_err(ERR_INVALID_HOTPLUG_TIMEOUT_KEY)),
            },
            TestData {
                param: "agent.hotplug_timeout=1",
                result: Ok(time::Duration::from_secs(1)),
            },
            TestData {
                param: "agent.hotplug_timeout=3",
                result: Ok(time::Duration::from_secs(3)),
            },
            TestData {
                param: "agent.hotplug_timeout=3600",
                result: Ok(time::Duration::from_secs(3600)),
            },
            TestData {
                param: "agent.hotplug_timeout=0",
                result: Ok(time::Duration::from_secs(0)),
            },
            TestData {
                param: "agent.hotplug_timeout=-1",
                result: Err(make_err(ERR_INVALID_HOTPLUG_TIMEOUT_PARAM)),
            },
            TestData {
                param: "agent.hotplug_timeout=4jbsdja",
                result: Err(make_err(ERR_INVALID_HOTPLUG_TIMEOUT_PARAM)),
            },
            TestData {
                param: "agent.hotplug_timeout=foo",
                result: Err(make_err(ERR_INVALID_HOTPLUG_TIMEOUT_PARAM)),
            },
            TestData {
                param: "agent.hotplug_timeout=j",
                result: Err(make_err(ERR_INVALID_HOTPLUG_TIMEOUT_PARAM)),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = get_hotplug_timeout(d.param);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, format!("{}", msg));
        }
    }

    #[test]
    fn test_get_container_pipe_size() {
        #[derive(Debug)]
        struct TestData<'a> {
            param: &'a str,
            result: Result<i32>,
        }

        let tests = &[
            TestData {
                param: "",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_SIZE)),
            },
            TestData {
                param: "agent.container_pipe_size",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_SIZE)),
            },
            TestData {
                param: "foo=bar",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_SIZE_KEY)),
            },
            TestData {
                param: "agent.container_pip_siz=1",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_SIZE_KEY)),
            },
            TestData {
                param: "agent.container_pipe_size=1",
                result: Ok(1),
            },
            TestData {
                param: "agent.container_pipe_size=3",
                result: Ok(3),
            },
            TestData {
                param: "agent.container_pipe_size=2097152",
                result: Ok(2097152),
            },
            TestData {
                param: "agent.container_pipe_size=0",
                result: Ok(0),
            },
            TestData {
                param: "agent.container_pipe_size=-1",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_NEGATIVE)),
            },
            TestData {
                param: "agent.container_pipe_size=foobar",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_SIZE_PARAM)),
            },
            TestData {
                param: "agent.container_pipe_size=j",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_SIZE_PARAM)),
            },
            TestData {
                param: "agent.container_pipe_size=4jbsdja",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_SIZE_PARAM)),
            },
            TestData {
                param: "agent.container_pipe_size=4294967296",
                result: Err(make_err(ERR_INVALID_CONTAINER_PIPE_SIZE_PARAM)),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = get_container_pipe_size(d.param);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, format!("{}", msg));
        }
    }
}
