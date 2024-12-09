// Copyright (C) 2023 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use crate::compact;
use crate::memcg::{self, MemCgroup};
use crate::{error, info};
use anyhow::{anyhow, Result};
use std::thread;
use tokio::runtime::{Builder, Runtime};
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration, Instant};

const AGENT_WORK_ERROR_SLEEP_SECS: u64 = 5 * 60;

#[derive(Debug)]
enum AgentCmd {
    MemcgStatus,
    MemcgSet(memcg::OptionConfig),
    CompactSet(compact::OptionConfig),
}

#[allow(dead_code)]
#[derive(Debug)]
enum AgentReturn {
    Ok,
    Err(anyhow::Error),
    MemcgStatus(Vec<memcg::MemCgroup>),
}

async fn handle_agent_cmd(
    cmd: AgentCmd,
    ret_tx: oneshot::Sender<AgentReturn>,
    memcg: &mut memcg::MemCG,
    comp: &mut compact::Compact,
) -> Result<bool> {
    #[allow(unused_assignments)]
    let mut ret_msg = AgentReturn::Ok;

    let need_reset_mas = match cmd {
        AgentCmd::MemcgStatus => {
            ret_msg = AgentReturn::MemcgStatus(memcg.get_status().await);
            false
        }
        AgentCmd::MemcgSet(opt) => memcg.set_config(opt).await,
        AgentCmd::CompactSet(opt) => comp.set_config(opt).await,
    };

    ret_tx
        .send(ret_msg)
        .map_err(|e| anyhow!("ret_tx.send failed: {:?}", e))?;

    Ok(need_reset_mas)
}

fn get_remaining_tokio_duration(memcg: &memcg::MemCG, comp: &compact::Compact) -> Duration {
    let memcg_d = memcg.get_remaining_tokio_duration();
    let comp_d = comp.get_remaining_tokio_duration();

    if memcg_d > comp_d {
        comp_d
    } else {
        memcg_d
    }
}

async fn async_get_remaining_tokio_duration(
    memcg: &memcg::MemCG,
    comp: &compact::Compact,
) -> Duration {
    let memcg_f = memcg.async_get_remaining_tokio_duration();
    let comp_f = comp.async_get_remaining_tokio_duration();

    let memcg_d = memcg_f.await;
    let comp_d = comp_f.await;

    if memcg_d > comp_d {
        comp_d
    } else {
        memcg_d
    }
}

fn agent_work(mut memcg: memcg::MemCG, mut comp: compact::Compact) -> Result<Duration> {
    let memcg_need_reset = if memcg.need_work() {
        info!("memcg.work start");
        memcg
            .work()
            .map_err(|e| anyhow!("memcg.work failed: {}", e))?;
        info!("memcg.work stop");
        true
    } else {
        false
    };

    let compact_need_reset = if comp.need_work() {
        info!("compact.work start");
        comp.work()
            .map_err(|e| anyhow!("comp.work failed: {}", e))?;
        info!("compact.work stop");
        true
    } else {
        false
    };

    if memcg_need_reset {
        memcg.reset_timer();
    }
    if compact_need_reset {
        comp.reset_timer();
    }

    Ok(get_remaining_tokio_duration(&memcg, &comp))
}

struct MemAgentSleep {
    duration: Duration,
    start_wait_time: Instant,
    timeout: bool,
}

impl MemAgentSleep {
    fn new() -> Self {
        Self {
            duration: Duration::MAX,
            start_wait_time: Instant::now(),
            timeout: true,
        }
    }

    fn set_timeout(&mut self) {
        self.duration = Duration::MAX;
        self.timeout = true;
    }

    fn set_sleep(&mut self, d: Duration) {
        self.duration = d;
        self.start_wait_time = Instant::now();
    }

    /* Return true if timeout */
    fn refresh(&mut self) -> bool {
        if self.duration != Duration::MAX {
            let elapsed = self.start_wait_time.elapsed();
            if self.duration > elapsed {
                self.duration -= elapsed;
            } else {
                /* timeout */
                self.set_timeout();
                return true;
            }
        }

        false
    }
}

async fn mem_agent_loop(
    mut cmd_rx: mpsc::Receiver<(AgentCmd, oneshot::Sender<AgentReturn>)>,
    mut memcg: memcg::MemCG,
    mut comp: compact::Compact,
) -> Result<()> {
    let (work_ret_tx, mut work_ret_rx) = mpsc::channel(2);
    // the time that wait to next.
    let mut mas = MemAgentSleep::new();

    loop {
        if mas.timeout {
            let thread_memcg = memcg.clone();
            let thread_comp = comp.clone();
            let thread_work_ret_tx = work_ret_tx.clone();
            thread::spawn(move || {
                info!("agent work thread start");
                let d = agent_work(thread_memcg, thread_comp).unwrap_or_else(|err| {
                    error!("agent work thread fail {}", err);
                    Duration::from_secs(AGENT_WORK_ERROR_SLEEP_SECS)
                });
                if let Err(e) = thread_work_ret_tx.blocking_send(d) {
                    error!("work_ret_tx.blocking_send failed: {}", e);
                }
            });

            mas.timeout = false;
        } else {
            if mas.refresh() {
                continue;
            }
        }

        info!("mem_agent_loop wait timeout {:?}", mas.duration);
        select! {
            Some((cmd, ret_tx)) = cmd_rx.recv() => {
                if handle_agent_cmd(cmd, ret_tx, &mut memcg, &mut comp).await.map_err(|e| anyhow!("handle_agent_cmd failed: {}", e))? && !mas.timeout{
                    mas.set_sleep(async_get_remaining_tokio_duration(&memcg, &comp).await);
                }
            }
            d = work_ret_rx.recv() => {
                info!("agent work thread stop");
                mas.set_sleep(d.unwrap_or(Duration::from_secs(AGENT_WORK_ERROR_SLEEP_SECS)));
            }
            _ = async {
                sleep(mas.duration).await;
            } => {
                mas.set_timeout();
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct MemAgent {
    cmd_tx: mpsc::Sender<(AgentCmd, oneshot::Sender<AgentReturn>)>,
}

impl MemAgent {
    pub fn new(
        memcg_config: memcg::Config,
        compact_config: compact::Config,
    ) -> Result<(Self, Runtime)> {
        let mg = memcg::MemCG::new(memcg_config)
            .map_err(|e| anyhow!("memcg::MemCG::new fail: {}", e))?;

        let comp = compact::Compact::new(compact_config)
            .map_err(|e| anyhow!("compact::Compact::new fail: {}", e))?;

        let (cmd_tx, cmd_rx) = mpsc::channel(10);

        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| anyhow!("Builder::new_multi_threa failed: {}", e))?;

        runtime.spawn(async move {
            info!("mem-agent start");
            match mem_agent_loop(cmd_rx, mg, comp).await {
                Err(e) => error!("mem-agent error {}", e),
                Ok(()) => info!("mem-agent stop"),
            }
        });

        Ok((Self { cmd_tx }, runtime))
    }

    async fn send_cmd_async(&self, cmd: AgentCmd) -> Result<AgentReturn> {
        let (ret_tx, ret_rx) = oneshot::channel();

        self.cmd_tx
            .send((cmd, ret_tx))
            .await
            .map_err(|e| anyhow!("cmd_tx.send cmd failed: {}", e))?;

        let ret = ret_rx
            .await
            .map_err(|e| anyhow!("ret_rx.recv failed: {}", e))?;

        Ok(ret)
    }

    pub async fn memcg_set_config_async(&self, opt: memcg::OptionConfig) -> Result<()> {
        let ret = self
            .send_cmd_async(AgentCmd::MemcgSet(opt))
            .await
            .map_err(|e| anyhow!("send_cmd failed: {}", e))?;

        match ret {
            AgentReturn::Err(e) => Err(anyhow!(
                "mem_agent thread memcg_set_config_async failed: {}",
                e
            )),
            AgentReturn::Ok => Ok(()),
            _ => Err(anyhow!(
                "mem_agent thread memcg_set_config_async return wrong value"
            )),
        }
    }

    pub async fn compact_set_config_async(&self, opt: compact::OptionConfig) -> Result<()> {
        let ret = self
            .send_cmd_async(AgentCmd::CompactSet(opt))
            .await
            .map_err(|e| anyhow!("send_cmd failed: {}", e))?;

        match ret {
            AgentReturn::Err(e) => Err(anyhow!(
                "mem_agent thread compact_set_config_async failed: {}",
                e
            )),
            AgentReturn::Ok => Ok(()),
            _ => Err(anyhow!(
                "mem_agent thread compact_set_config_async return wrong value"
            )),
        }
    }

    pub async fn memcg_status_async(&self) -> Result<Vec<MemCgroup>> {
        let ret = self
            .send_cmd_async(AgentCmd::MemcgStatus)
            .await
            .map_err(|e| anyhow!("send_cmd failed: {}", e))?;

        let status = match ret {
            AgentReturn::Err(e) => {
                return Err(anyhow!("mem_agent thread memcg_status_async failed: {}", e))
            }
            AgentReturn::Ok => {
                return Err(anyhow!(
                    "mem_agent thread memcg_status_async return wrong value"
                ))
            }
            AgentReturn::MemcgStatus(s) => s,
        };

        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent() {
        let memcg_config = memcg::Config {
            disabled: true,
            ..Default::default()
        };
        let compact_config = compact::Config {
            disabled: true,
            ..Default::default()
        };

        let (ma, _rt) = MemAgent::new(memcg_config, compact_config).unwrap();

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on({
                let memcg_config = memcg::OptionConfig {
                    period_secs: Some(120),
                    ..Default::default()
                };
                ma.memcg_set_config_async(memcg_config)
            })
            .unwrap();

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on({
                let compact_config = compact::OptionConfig {
                    period_secs: Some(280),
                    ..Default::default()
                };
                ma.compact_set_config_async(compact_config)
            })
            .unwrap();
    }

    #[test]
    fn test_agent_memcg_status() {
        let memcg_config = memcg::Config {
            disabled: true,
            ..Default::default()
        };
        let compact_config = compact::Config {
            disabled: true,
            ..Default::default()
        };

        let (ma, _rt) = MemAgent::new(memcg_config, compact_config).unwrap();

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(ma.memcg_status_async())
            .unwrap();
    }
}
