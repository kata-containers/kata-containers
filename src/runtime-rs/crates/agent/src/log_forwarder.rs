// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::sock;

// https://github.com/slog-rs/slog/blob/master/src/lib.rs#L2082
const LOG_LEVEL_TRACE: &str = "TRCE";
const LOG_LEVEL_DEBUG: &str = "DEBG";
const LOG_LEVEL_INFO: &str = "INFO";
const LOG_LEVEL_WARNING: &str = "WARN";
const LOG_LEVEL_ERROR: &str = "ERRO";
const LOG_LEVEL_CRITICAL: &str = "CRIT";

pub(crate) struct LogForwarder {
    task_handler: Option<tokio::task::JoinHandle<()>>,
}

impl LogForwarder {
    pub(crate) fn new() -> Self {
        Self { task_handler: None }
    }

    pub(crate) fn stop(&mut self) {
        let task_handler = self.task_handler.take();
        if let Some(handler) = task_handler {
            handler.abort();
            info!(sl!(), "abort log forwarder thread");
        }
    }

    // start connect kata-agent log vsock and copy data to hypervisor's log stream
    pub(crate) async fn start(
        &mut self,
        address: &str,
        port: u32,
        config: sock::ConnectConfig,
    ) -> Result<()> {
        let logger = sl!().clone();
        let address = address.to_string();
        let task_handler = tokio::spawn(async move {
            loop {
                info!(logger, "try to connect to get agent log");
                let sock = match sock::new(&address, port) {
                    Ok(sock) => sock,
                    Err(err) => {
                        error!(
                            sl!(),
                            "failed to new sock for address {:?} port {} error {:?}",
                            address,
                            port,
                            err
                        );
                        return;
                    }
                };

                match sock.connect(&config).await {
                    Ok(stream) => {
                        let stream = BufReader::new(stream);
                        let mut lines = stream.lines();
                        while let Ok(Some(l)) = lines.next_line().await {
                            match parse_agent_log_level(&l) {
                                LOG_LEVEL_TRACE => trace!(sl!(), "{}", l),
                                LOG_LEVEL_DEBUG => debug!(sl!(), "{}", l),
                                LOG_LEVEL_WARNING => warn!(sl!(), "{}", l),
                                LOG_LEVEL_ERROR => error!(sl!(), "{}", l),
                                LOG_LEVEL_CRITICAL => crit!(sl!(), "{}", l),
                                _ => info!(sl!(), "{}", l),
                            }
                        }
                    }
                    Err(err) => {
                        warn!(logger, "connect agent vsock failed: {:?}", err);
                    }
                }
            }
        });
        self.task_handler = Some(task_handler);
        Ok(())
    }
}

pub fn parse_agent_log_level(s: &str) -> &str {
    let v: serde_json::Result<serde_json::Value> = serde_json::from_str(s);
    match v {
        Err(_err) => LOG_LEVEL_INFO,
        Ok(val) => {
            match &val["level"] {
                serde_json::Value::String(s) => match s.as_str() {
                    LOG_LEVEL_TRACE => LOG_LEVEL_TRACE,
                    LOG_LEVEL_DEBUG => LOG_LEVEL_DEBUG,
                    LOG_LEVEL_WARNING => LOG_LEVEL_WARNING,
                    LOG_LEVEL_ERROR => LOG_LEVEL_ERROR,
                    LOG_LEVEL_CRITICAL => LOG_LEVEL_CRITICAL,
                    _ => LOG_LEVEL_INFO, // info or other values will return info,
                },
                _ => LOG_LEVEL_INFO, // info or other values will return info,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_agent_log_level;

    #[test]
    fn test_parse_agent_log_level() {
        let cases = vec![
            // normal cases
            (
                r#"{"msg":"child exited unexpectedly","level":"TRCE"}"#,
                super::LOG_LEVEL_TRACE,
            ),
            (
                r#"{"msg":"child exited unexpectedly","level":"DEBG"}"#,
                super::LOG_LEVEL_DEBUG,
            ),
            (
                r#"{"msg":"child exited unexpectedly","level":"INFO"}"#,
                super::LOG_LEVEL_INFO,
            ),
            (
                r#"{"msg":"child exited unexpectedly","level":"WARN"}"#,
                super::LOG_LEVEL_WARNING,
            ),
            (
                r#"{"msg":"child exited unexpectedly","level":"ERRO"}"#,
                super::LOG_LEVEL_ERROR,
            ),
            (
                r#"{"msg":"child exited unexpectedly","level":"CRIT"}"#,
                super::LOG_LEVEL_CRITICAL,
            ),
            (
                r#"{"msg":"child exited unexpectedly","level":"abc"}"#,
                super::LOG_LEVEL_INFO,
            ),
            // exception cases
            (r#"{"not a valid json struct"}"#, super::LOG_LEVEL_INFO),
            ("not a valid json struct", super::LOG_LEVEL_INFO),
        ];

        for case in cases.iter() {
            let s = case.0;
            let result = parse_agent_log_level(s);
            let excepted = case.1;
            assert_eq!(result, excepted);
        }
    }
}
