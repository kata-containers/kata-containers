// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//
use crate::tracer;
use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::time;
use tracing::instrument;

const DEBUG_CONSOLE_FLAG: &str = "agent.debug_console";
const DEV_MODE_FLAG: &str = "agent.devmode";
const TRACE_MODE_OPTION: &str = "agent.trace";
const LOG_LEVEL_OPTION: &str = "agent.log";
const SERVER_ADDR_OPTION: &str = "agent.server_addr";
const HOTPLUG_TIMOUT_OPTION: &str = "agent.hotplug_timeout";
const DEBUG_CONSOLE_VPORT_OPTION: &str = "agent.debug_console_vport";
const LOG_VPORT_OPTION: &str = "agent.log_vport";
const CONTAINER_PIPE_SIZE_OPTION: &str = "agent.container_pipe_size";
const UNIFIED_CGROUP_HIERARCHY_OPTION: &str = "agent.unified_cgroup_hierarchy";

const DEFAULT_LOG_LEVEL: slog::Level = slog::Level::Info;
const DEFAULT_HOTPLUG_TIMEOUT: time::Duration = time::Duration::from_secs(3);
const DEFAULT_CONTAINER_PIPE_SIZE: i32 = 0;
const VSOCK_ADDR: &str = "vsock://-1";
const VSOCK_PORT: u16 = 1024;

// Environment variables used for development and testing
const SERVER_ADDR_ENV_VAR: &str = "KATA_AGENT_SERVER_ADDR";
const LOG_LEVEL_ENV_VAR: &str = "KATA_AGENT_LOG_LEVEL";
const TRACE_TYPE_ENV_VAR: &str = "KATA_AGENT_TRACE_TYPE";

const ERR_INVALID_LOG_LEVEL: &str = "invalid log level";
const ERR_INVALID_LOG_LEVEL_PARAM: &str = "invalid log level parameter";
const ERR_INVALID_GET_VALUE_PARAM: &str = "expected name=value";
const ERR_INVALID_GET_VALUE_NO_NAME: &str = "name=value parameter missing name";
const ERR_INVALID_GET_VALUE_NO_VALUE: &str = "name=value parameter missing value";
const ERR_INVALID_LOG_LEVEL_KEY: &str = "invalid log level key name";

const ERR_INVALID_HOTPLUG_TIMEOUT: &str = "invalid hotplug timeout parameter";
const ERR_INVALID_HOTPLUG_TIMEOUT_PARAM: &str = "unable to parse hotplug timeout";
const ERR_INVALID_HOTPLUG_TIMEOUT_KEY: &str = "invalid hotplug timeout key name";

const ERR_INVALID_CONTAINER_PIPE_SIZE: &str = "invalid container pipe size parameter";
const ERR_INVALID_CONTAINER_PIPE_SIZE_PARAM: &str = "unable to parse container pipe size";
const ERR_INVALID_CONTAINER_PIPE_SIZE_KEY: &str = "invalid container pipe size key name";
const ERR_INVALID_CONTAINER_PIPE_NEGATIVE: &str = "container pipe size should not be negative";

#[derive(Debug)]
pub struct AgentConfig {
    pub debug_console: bool,
    pub dev_mode: bool,
    pub log_level: slog::Level,
    pub hotplug_timeout: time::Duration,
    pub debug_console_vport: i32,
    pub log_vport: i32,
    pub container_pipe_size: i32,
    pub server_addr: String,
    pub unified_cgroup_hierarchy: bool,
    pub tracing: tracer::TraceType,
}

// parse_cmdline_param parse commandline parameters.
macro_rules! parse_cmdline_param {
    // commandline flags, without func to parse the option values
    ($param:ident, $key:ident, $field:expr) => {
        if $param.eq(&$key) {
            $field = true;
            continue;
        }
    };
    // commandline options, with func to parse the option values
    ($param:ident, $key:ident, $field:expr, $func:ident) => {
        if $param.starts_with(format!("{}=", $key).as_str()) {
            let val = $func($param)?;
            $field = val;
            continue;
        }
    };
    // commandline options, with func to parse the option values, and match func
    // to valid the values
    ($param:ident, $key:ident, $field:expr, $func:ident, $guard:expr) => {
        if $param.starts_with(format!("{}=", $key).as_str()) {
            let val = $func($param)?;
            if $guard(val) {
                $field = val;
            }
            continue;
        }
    };
}

impl AgentConfig {
    pub fn new() -> AgentConfig {
        AgentConfig {
            debug_console: false,
            dev_mode: false,
            log_level: DEFAULT_LOG_LEVEL,
            hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
            debug_console_vport: 0,
            log_vport: 0,
            container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
            server_addr: format!("{}:{}", VSOCK_ADDR, VSOCK_PORT),
            unified_cgroup_hierarchy: false,
            tracing: tracer::TraceType::Disabled,
        }
    }

    #[instrument]
    pub fn parse_cmdline(&mut self, file: &str) -> Result<()> {
        let cmdline = fs::read_to_string(file)?;
        let params: Vec<&str> = cmdline.split_ascii_whitespace().collect();
        for param in params.iter() {
            // parse cmdline flags
            parse_cmdline_param!(param, DEBUG_CONSOLE_FLAG, self.debug_console);
            parse_cmdline_param!(param, DEV_MODE_FLAG, self.dev_mode);

            // Support "bare" tracing option for backwards compatibility with
            // Kata 1.x.
            if param == &TRACE_MODE_OPTION {
                self.tracing = tracer::TraceType::Isolated;
                continue;
            }

            parse_cmdline_param!(param, TRACE_MODE_OPTION, self.tracing, get_trace_type);

            // parse cmdline options
            parse_cmdline_param!(param, LOG_LEVEL_OPTION, self.log_level, get_log_level);
            parse_cmdline_param!(
                param,
                SERVER_ADDR_OPTION,
                self.server_addr,
                get_string_value
            );

            // ensure the timeout is a positive value
            parse_cmdline_param!(
                param,
                HOTPLUG_TIMOUT_OPTION,
                self.hotplug_timeout,
                get_hotplug_timeout,
                |hotplug_timeout: time::Duration| hotplug_timeout.as_secs() > 0
            );

            // vsock port should be positive values
            parse_cmdline_param!(
                param,
                DEBUG_CONSOLE_VPORT_OPTION,
                self.debug_console_vport,
                get_vsock_port,
                |port| port > 0
            );
            parse_cmdline_param!(
                param,
                LOG_VPORT_OPTION,
                self.log_vport,
                get_vsock_port,
                |port| port > 0
            );

            parse_cmdline_param!(
                param,
                CONTAINER_PIPE_SIZE_OPTION,
                self.container_pipe_size,
                get_container_pipe_size
            );
            parse_cmdline_param!(
                param,
                UNIFIED_CGROUP_HIERARCHY_OPTION,
                self.unified_cgroup_hierarchy,
                get_bool_value
            );
        }

        if let Ok(addr) = env::var(SERVER_ADDR_ENV_VAR) {
            self.server_addr = addr;
        }

        if let Ok(addr) = env::var(LOG_LEVEL_ENV_VAR) {
            if let Ok(level) = logrus_to_slog_level(&addr) {
                self.log_level = level;
            }
        }

        if let Ok(value) = env::var(TRACE_TYPE_ENV_VAR) {
            if let Ok(result) = value.parse::<tracer::TraceType>() {
                self.tracing = result;
            }
        }

        Ok(())
    }
}

#[instrument]
fn get_vsock_port(p: &str) -> Result<i32> {
    let fields: Vec<&str> = p.split('=').collect();
    if fields.len() != 2 {
        return Err(anyhow!("invalid port parameter"));
    }

    Ok(fields[1].parse::<i32>()?)
}

// Map logrus (https://godoc.org/github.com/sirupsen/logrus)
// log level to the equivalent slog log levels.
//
// Note: Logrus names are used for compatability with the previous
// golang-based agent.
#[instrument]
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
            return Err(anyhow!(ERR_INVALID_LOG_LEVEL));
        }
    };

    Ok(level)
}

#[instrument]
fn get_log_level(param: &str) -> Result<slog::Level> {
    let fields: Vec<&str> = param.split('=').collect();

    if fields.len() != 2 {
        return Err(anyhow!(ERR_INVALID_LOG_LEVEL_PARAM));
    }

    if fields[0] != LOG_LEVEL_OPTION {
        Err(anyhow!(ERR_INVALID_LOG_LEVEL_KEY))
    } else {
        Ok(logrus_to_slog_level(fields[1])?)
    }
}

#[instrument]
fn get_trace_type(param: &str) -> Result<tracer::TraceType> {
    if param.is_empty() {
        return Err(anyhow!("invalid trace type parameter"));
    }

    let fields: Vec<&str> = param.split('=').collect();

    if fields[0] != TRACE_MODE_OPTION {
        return Err(anyhow!("invalid trace type key name"));
    }

    if fields.len() == 1 {
        return Ok(tracer::TraceType::Isolated);
    }

    let result = fields[1].parse::<tracer::TraceType>()?;

    Ok(result)
}

#[instrument]
fn get_hotplug_timeout(param: &str) -> Result<time::Duration> {
    let fields: Vec<&str> = param.split('=').collect();

    if fields.len() != 2 {
        return Err(anyhow!(ERR_INVALID_HOTPLUG_TIMEOUT));
    }

    let key = fields[0];
    if key != HOTPLUG_TIMOUT_OPTION {
        return Err(anyhow!(ERR_INVALID_HOTPLUG_TIMEOUT_KEY));
    }

    let value = fields[1].parse::<u64>();
    if value.is_err() {
        return Err(anyhow!(ERR_INVALID_HOTPLUG_TIMEOUT_PARAM));
    }

    Ok(time::Duration::from_secs(value.unwrap()))
}

#[instrument]
fn get_bool_value(param: &str) -> Result<bool> {
    let fields: Vec<&str> = param.split('=').collect();

    if fields.len() != 2 {
        return Ok(false);
    }

    let v = fields[1];

    // first try to parse as bool value
    v.parse::<bool>().or_else(|_err1| {
        // then try to parse as integer value
        v.parse::<u64>().or(Ok(0)).map(|v| !matches!(v, 0))
    })
}

// Return the value from a "name=value" string.
//
// Note:
//
// - A name *and* a value is required.
// - A value can contain any number of equal signs.
// - We could/should maybe check if the name is pure whitespace
//   since this is considered to be invalid.
#[instrument]
fn get_string_value(param: &str) -> Result<String> {
    let fields: Vec<&str> = param.split('=').collect();

    if fields.len() < 2 {
        return Err(anyhow!(ERR_INVALID_GET_VALUE_PARAM));
    }

    // We need name (but the value can be blank)
    if fields[0].is_empty() {
        return Err(anyhow!(ERR_INVALID_GET_VALUE_NO_NAME));
    }

    let value = fields[1..].join("=");
    if value.is_empty() {
        return Err(anyhow!(ERR_INVALID_GET_VALUE_NO_VALUE));
    }

    Ok(value)
}

#[instrument]
fn get_container_pipe_size(param: &str) -> Result<i32> {
    let fields: Vec<&str> = param.split('=').collect();

    if fields.len() != 2 {
        return Err(anyhow!(ERR_INVALID_CONTAINER_PIPE_SIZE));
    }

    let key = fields[0];
    if key != CONTAINER_PIPE_SIZE_OPTION {
        return Err(anyhow!(ERR_INVALID_CONTAINER_PIPE_SIZE_KEY));
    }

    let res = fields[1].parse::<i32>();
    if res.is_err() {
        return Err(anyhow!(ERR_INVALID_CONTAINER_PIPE_SIZE_PARAM));
    }

    let value = res.unwrap();
    if value < 0 {
        return Err(anyhow!(ERR_INVALID_CONTAINER_PIPE_NEGATIVE));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Error;
    use std::fs::File;
    use std::io::Write;
    use std::time;
    use tempfile::tempdir;

    const ERR_INVALID_TRACE_TYPE_PARAM: &str = "invalid trace type parameter";
    const ERR_INVALID_TRACE_TYPE: &str = "invalid trace type";
    const ERR_INVALID_TRACE_TYPE_KEY: &str = "invalid trace type key name";

    // helper function to make errors less crazy-long
    fn make_err(desc: &str) -> Error {
        anyhow!(desc.to_string())
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
                assert!(*expected_level == actual_level, "{}", $msg);
            } else {
                let expected_error = $expected_result.as_ref().unwrap_err();
                let expected_error_msg = format!("{:?}", expected_error);

                if let Err(actual_error) = $actual_result {
                    let actual_error_msg = format!("{:?}", actual_error);

                    assert!(expected_error_msg == actual_error_msg, "{}", $msg);
                } else {
                    assert!(expected_error_msg == "expected error, got OK", "{}", $msg);
                }
            }
        };
    }

    #[test]
    fn test_new() {
        let config = AgentConfig::new();
        assert_eq!(config.debug_console, false);
        assert_eq!(config.dev_mode, false);
        assert_eq!(config.log_level, DEFAULT_LOG_LEVEL);
        assert_eq!(config.hotplug_timeout, DEFAULT_HOTPLUG_TIMEOUT);
    }

    #[test]
    fn test_parse_cmdline() {
        const TEST_SERVER_ADDR: &str = "vsock://-1:1024";

        #[derive(Debug)]
        struct TestData<'a> {
            contents: &'a str,
            env_vars: Vec<&'a str>,
            debug_console: bool,
            dev_mode: bool,
            log_level: slog::Level,
            hotplug_timeout: time::Duration,
            container_pipe_size: i32,
            server_addr: &'a str,
            unified_cgroup_hierarchy: bool,
            tracing: tracer::TraceType,
        }

        let tests = &[
            TestData {
                contents: "agent.debug_consolex agent.devmode",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.debug_console agent.devmodex",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.logx=debug",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.log=debug",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: slog::Level::Debug,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.log=debug",
                env_vars: vec!["KATA_AGENT_LOG_LEVEL=trace"],
                debug_console: false,
                dev_mode: false,
                log_level: slog::Level::Trace,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo bar",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo bar",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo agent bar",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo debug_console agent bar devmode",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.debug_console",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "   agent.debug_console ",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.debug_console foo",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: " agent.debug_console foo",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo agent.debug_console bar",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo agent.debug_console",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo agent.debug_console ",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "   agent.devmode ",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode foo",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: " agent.devmode foo",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo agent.devmode bar",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo agent.devmode",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "foo agent.devmode ",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode agent.debug_console",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.hotplug_timeout=100 agent.unified_cgroup_hierarchy=a",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: time::Duration::from_secs(100),
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.hotplug_timeout=0 agent.unified_cgroup_hierarchy=11",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: true,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.container_pipe_size=2097152 agent.unified_cgroup_hierarchy=false",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: 2097152,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.container_pipe_size=100 agent.unified_cgroup_hierarchy=true",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: 100,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: true,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.container_pipe_size=0 agent.unified_cgroup_hierarchy=0",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.devmode agent.debug_console agent.container_pip_siz=100 agent.unified_cgroup_hierarchy=1",
                env_vars: Vec::new(),
                debug_console: true,
                dev_mode: true,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: true,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_SERVER_ADDR=foo"],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "foo",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_SERVER_ADDR=="],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "=",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_SERVER_ADDR==foo"],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "=foo",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_SERVER_ADDR=foo=bar=baz="],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "foo=bar=baz=",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_SERVER_ADDR=unix:///tmp/foo.socket"],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "unix:///tmp/foo.socket",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_SERVER_ADDR=unix://@/tmp/foo.socket"],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "unix://@/tmp/foo.socket",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_LOG_LEVEL="],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_LOG_LEVEL=invalid"],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_LOG_LEVEL=debug"],
                debug_console: false,
                dev_mode: false,
                log_level: slog::Level::Debug,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_LOG_LEVEL=debugger"],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "server_addr=unix:///tmp/foo.socket",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.server_address=unix:///tmp/foo.socket",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.server_addr=unix:///tmp/foo.socket",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "unix:///tmp/foo.socket",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: " agent.server_addr=unix:///tmp/foo.socket",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "unix:///tmp/foo.socket",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: " agent.server_addr=unix:///tmp/foo.socket a",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: "unix:///tmp/foo.socket",
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "trace",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: ".trace",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.tracer",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.trac",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "agent.trace",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Isolated,
            },
            TestData {
                contents: "agent.trace=isolated",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Isolated,
            },
            TestData {
                contents: "agent.trace=disabled",
                env_vars: Vec::new(),
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_TRACE_TYPE=isolated"],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Isolated,
            },
            TestData {
                contents: "",
                env_vars: vec!["KATA_AGENT_TRACE_TYPE=disabled"],
                debug_console: false,
                dev_mode: false,
                log_level: DEFAULT_LOG_LEVEL,
                hotplug_timeout: DEFAULT_HOTPLUG_TIMEOUT,
                container_pipe_size: DEFAULT_CONTAINER_PIPE_SIZE,
                server_addr: TEST_SERVER_ADDR,
                unified_cgroup_hierarchy: false,
                tracing: tracer::TraceType::Disabled,
            },
        ];

        let dir = tempdir().expect("failed to create tmpdir");

        // First, check a missing file is handled
        let file_path = dir.path().join("enoent");

        let filename = file_path.to_str().expect("failed to create filename");

        let mut config = AgentConfig::new();
        let result = config.parse_cmdline(&filename.to_owned());
        assert!(result.is_err());

        // Now, test various combinations of file contents and environment
        // variables.
        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let file_path = dir.path().join("cmdline");

            let filename = file_path.to_str().expect("failed to create filename");

            let mut file =
                File::create(filename).unwrap_or_else(|_| panic!("{}: failed to create file", msg));

            file.write_all(d.contents.as_bytes())
                .unwrap_or_else(|_| panic!("{}: failed to write file contents", msg));

            let mut vars_to_unset = Vec::new();

            for v in &d.env_vars {
                let fields: Vec<&str> = v.split('=').collect();

                let name = fields[0];
                let value = fields[1..].join("=");

                env::set_var(name, value);

                vars_to_unset.push(name);
            }

            let mut config = AgentConfig::new();
            assert_eq!(config.debug_console, false, "{}", msg);
            assert_eq!(config.dev_mode, false, "{}", msg);
            assert_eq!(config.unified_cgroup_hierarchy, false, "{}", msg);
            assert_eq!(
                config.hotplug_timeout,
                time::Duration::from_secs(3),
                "{}",
                msg
            );
            assert_eq!(config.container_pipe_size, 0, "{}", msg);
            assert_eq!(config.server_addr, TEST_SERVER_ADDR, "{}", msg);
            assert_eq!(config.tracing, tracer::TraceType::Disabled, "{}", msg);

            let result = config.parse_cmdline(filename);
            assert!(result.is_ok(), "{}", msg);

            assert_eq!(d.debug_console, config.debug_console, "{}", msg);
            assert_eq!(d.dev_mode, config.dev_mode, "{}", msg);
            assert_eq!(
                d.unified_cgroup_hierarchy, config.unified_cgroup_hierarchy,
                "{}",
                msg
            );
            assert_eq!(d.log_level, config.log_level, "{}", msg);
            assert_eq!(d.hotplug_timeout, config.hotplug_timeout, "{}", msg);
            assert_eq!(d.container_pipe_size, config.container_pipe_size, "{}", msg);
            assert_eq!(d.server_addr, config.server_addr, "{}", msg);
            assert_eq!(d.tracing, config.tracing, "{}", msg);

            for v in vars_to_unset {
                env::remove_var(v);
            }
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

            assert_result!(d.result, result, msg);
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

            assert_result!(d.result, result, msg);
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

            assert_result!(d.result, result, msg);
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

            assert_result!(d.result, result, msg);
        }
    }

    #[test]
    fn test_get_string_value() {
        #[derive(Debug)]
        struct TestData<'a> {
            param: &'a str,
            result: Result<String>,
        }

        let tests = &[
            TestData {
                param: "",
                result: Err(make_err(ERR_INVALID_GET_VALUE_PARAM)),
            },
            TestData {
                param: "=",
                result: Err(make_err(ERR_INVALID_GET_VALUE_NO_NAME)),
            },
            TestData {
                param: "==",
                result: Err(make_err(ERR_INVALID_GET_VALUE_NO_NAME)),
            },
            TestData {
                param: "x=",
                result: Err(make_err(ERR_INVALID_GET_VALUE_NO_VALUE)),
            },
            TestData {
                param: "x==",
                result: Ok("=".into()),
            },
            TestData {
                param: "x===",
                result: Ok("==".into()),
            },
            TestData {
                param: "x==x",
                result: Ok("=x".into()),
            },
            TestData {
                param: "x=x",
                result: Ok("x".into()),
            },
            TestData {
                param: "x=x=",
                result: Ok("x=".into()),
            },
            TestData {
                param: "x=x=x",
                result: Ok("x=x".into()),
            },
            TestData {
                param: "foo=bar",
                result: Ok("bar".into()),
            },
            TestData {
                param: "x= =",
                result: Ok(" =".into()),
            },
            TestData {
                param: "x= =",
                result: Ok(" =".into()),
            },
            TestData {
                param: "x= = ",
                result: Ok(" = ".into()),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = get_string_value(d.param);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }

    #[test]
    fn test_get_trace_type() {
        #[derive(Debug)]
        struct TestData<'a> {
            param: &'a str,
            result: Result<tracer::TraceType>,
        }

        let tests = &[
            TestData {
                param: "",
                result: Err(make_err(ERR_INVALID_TRACE_TYPE_PARAM)),
            },
            TestData {
                param: "agent.tracer",
                result: Err(make_err(ERR_INVALID_TRACE_TYPE_KEY)),
            },
            TestData {
                param: "agent.trac",
                result: Err(make_err(ERR_INVALID_TRACE_TYPE_KEY)),
            },
            TestData {
                param: "agent.trace=",
                result: Err(make_err(ERR_INVALID_TRACE_TYPE)),
            },
            TestData {
                param: "agent.trace==",
                result: Err(make_err(ERR_INVALID_TRACE_TYPE)),
            },
            TestData {
                param: "agent.trace=foo",
                result: Err(make_err(ERR_INVALID_TRACE_TYPE)),
            },
            TestData {
                param: "agent.trace",
                result: Ok(tracer::TraceType::Isolated),
            },
            TestData {
                param: "agent.trace=isolated",
                result: Ok(tracer::TraceType::Isolated),
            },
            TestData {
                param: "agent.trace=disabled",
                result: Ok(tracer::TraceType::Disabled),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = get_trace_type(d.param);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }
}
