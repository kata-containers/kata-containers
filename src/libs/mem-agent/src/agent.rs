// Copyright (C) 2023 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use crate::compact;
use crate::memcg::{self, MemCgroup};
use crate::{debug, error, info};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
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
    Busy,
    Err(anyhow::Error),
    MemcgStatus(HashMap<String, memcg::MemCgroup>),
}

#[derive(Debug)]
pub struct MemAgentBusy;

impl std::fmt::Display for MemAgentBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mem-agent worker is busy; retry the configuration update"
        )
    }
}

impl std::error::Error for MemAgentBusy {}

fn send_busy_reply(ret_tx: oneshot::Sender<AgentReturn>) {
    if ret_tx.send(AgentReturn::Busy).is_err() {
        // The RPC may have been cancelled after queuing the command. A dropped
        // reply receiver must not stop the long-lived mem-agent loop.
        debug!("mem-agent busy reply receiver was dropped");
    }
}

type MemcgInitializer = Arc<dyn Fn() -> Result<()> + Send + Sync>;

#[derive(Clone)]
enum MemcgState {
    Disabled(memcg::Config),
    Ready(memcg::MemCG),
}

impl MemcgState {
    fn new(is_cg_v2: bool, config: memcg::Config, initializer: &MemcgInitializer) -> Result<Self> {
        if config.is_disabled() {
            return Ok(Self::Disabled(config));
        }

        initializer().context("prepare memcg environment")?;
        let memcg = memcg::MemCG::new(is_cg_v2, config)
            .map_err(|e| anyhow!("memcg::MemCG::new fail: {e}"))?;
        Ok(Self::Ready(memcg))
    }

    async fn set_config(
        &mut self,
        option: memcg::OptionConfig,
        is_cg_v2: bool,
        initializer: &MemcgInitializer,
    ) -> Result<bool> {
        match self {
            Self::Disabled(config) => {
                let (new_config, _) = config.updated(option)?;
                if new_config.is_disabled() {
                    *config = new_config;
                    return Ok(false);
                }

                initializer().context("prepare memcg environment")?;
                let memcg = memcg::MemCG::new(is_cg_v2, new_config)
                    .map_err(|e| anyhow!("memcg::MemCG::new fail: {e}"))?;
                *self = Self::Ready(memcg);
                Ok(true)
            }
            Self::Ready(memcg) => {
                let (need_reset, config) = memcg.set_config(option).await?;
                if config.is_disabled() {
                    *self = Self::Disabled(config);
                    Ok(true)
                } else {
                    Ok(need_reset)
                }
            }
        }
    }

    async fn get_status(&self) -> Result<HashMap<String, memcg::MemCgroup>> {
        match self {
            Self::Disabled(_) => Ok(HashMap::new()),
            Self::Ready(memcg) => memcg.get_status().await,
        }
    }

    fn get_remaining_tokio_duration(&self) -> Duration {
        match self {
            Self::Disabled(_) => Duration::MAX,
            Self::Ready(memcg) => memcg.get_remaining_tokio_duration(),
        }
    }

    async fn async_get_remaining_tokio_duration(&self) -> Duration {
        match self {
            Self::Disabled(_) => Duration::MAX,
            Self::Ready(memcg) => memcg.async_get_remaining_tokio_duration().await,
        }
    }
}

async fn handle_agent_cmd(
    cmd: AgentCmd,
    ret_tx: oneshot::Sender<AgentReturn>,
    memcg: &mut MemcgState,
    comp: &mut compact::Compact,
    is_cg_v2: bool,
    memcg_initializer: &MemcgInitializer,
) -> Result<bool> {
    #[allow(unused_assignments)]
    let mut ret_msg = AgentReturn::Ok;

    let need_reset_mas = match cmd {
        AgentCmd::MemcgStatus => {
            ret_msg = match memcg.get_status().await {
                Ok(status) => AgentReturn::MemcgStatus(status),
                Err(e) => AgentReturn::Err(e),
            };
            false
        }
        AgentCmd::MemcgSet(opt) => match memcg.set_config(opt, is_cg_v2, memcg_initializer).await {
            Ok(reset) => {
                ret_msg = AgentReturn::Ok;
                reset
            }
            Err(e) => {
                ret_msg = AgentReturn::Err(e);
                false
            }
        },
        AgentCmd::CompactSet(opt) => match comp.set_config(opt).await {
            Ok(reset) => reset,
            Err(e) => {
                ret_msg = AgentReturn::Err(e);
                false
            }
        },
    };

    if ret_tx.send(ret_msg).is_err() {
        debug!("mem-agent reply receiver was dropped");
    }

    Ok(need_reset_mas)
}

fn get_remaining_tokio_duration(memcg: &MemcgState, comp: &compact::Compact) -> Duration {
    let memcg_d = memcg.get_remaining_tokio_duration();
    let comp_d = comp.get_remaining_tokio_duration();

    debug!(
        "get_remaining_tokio_duration: memcg_d={:?}, comp_d={:?}",
        memcg_d, comp_d
    );

    if memcg_d > comp_d {
        comp_d
    } else {
        memcg_d
    }
}

async fn async_get_remaining_tokio_duration(
    memcg: &MemcgState,
    comp: &compact::Compact,
) -> Duration {
    let memcg_f = memcg.async_get_remaining_tokio_duration();
    let comp_f = comp.async_get_remaining_tokio_duration();

    let memcg_d = memcg_f.await;
    let comp_d = comp_f.await;

    debug!(
        "async_get_remaining_tokio_duration: memcg_d={:?}, comp_d={:?}",
        memcg_d, comp_d
    );

    if memcg_d > comp_d {
        comp_d
    } else {
        memcg_d
    }
}

fn agent_work(mut memcg: MemcgState, mut comp: compact::Compact) -> Result<Duration> {
    if let MemcgState::Ready(memcg) = &mut memcg {
        let memcg_work_list = memcg.get_timeout_list();
        if !memcg_work_list.is_empty() {
            info!("memcg.work start");
            memcg
                .work(&memcg_work_list)
                .map_err(|e| anyhow!("memcg.work failed: {e}"))?;
            info!("memcg.work stop");
        }

        memcg.reset_timers(&memcg_work_list);
    }

    let compact_need_reset = if comp.need_work() {
        info!("compact.work start");
        comp.work().map_err(|e| anyhow!("comp.work failed: {e}"))?;
        info!("compact.work stop");
        true
    } else {
        false
    };

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
        info!("MemAgentSleep::set_sleep: {:?}", d);

        self.duration = d;
        self.start_wait_time = Instant::now();
        self.timeout = false;
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
    mut memcg: MemcgState,
    mut comp: compact::Compact,
    is_cg_v2: bool,
    memcg_initializer: MemcgInitializer,
) -> Result<()> {
    let (work_ret_tx, mut work_ret_rx) = mpsc::channel(2);
    // the time that wait to next.
    let mut mas = MemAgentSleep::new();
    let mut work_in_flight = false;

    loop {
        if mas.timeout && !work_in_flight {
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
            work_in_flight = true;
        } else if !mas.timeout && mas.refresh() {
            continue;
        }

        info!("mem_agent_loop wait timeout {:?}", mas.duration);
        select! {
            Some((cmd, ret_tx)) = cmd_rx.recv() => {
                // Do not apply configuration while a worker clone is active.
                // The caller can retry after the current work cycle finishes.
                if matches!(&cmd, AgentCmd::MemcgSet(_)) && work_in_flight {
                    send_busy_reply(ret_tx);
                    continue;
                }

                if handle_agent_cmd(
                    cmd,
                    ret_tx,
                    &mut memcg,
                    &mut comp,
                    is_cg_v2,
                    &memcg_initializer,
                )
                .await
                .map_err(|e| anyhow!("handle_agent_cmd failed: {e}"))?
                    && !mas.timeout
                {
                    mas.set_sleep(async_get_remaining_tokio_duration(&memcg, &comp).await);
                }
            }
            d = work_ret_rx.recv() => {
                info!("agent work thread stop");
                work_in_flight = false;
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
    pub fn new<F>(
        memcg_config: memcg::Config,
        compact_config: compact::Config,
        memcg_initializer: F,
    ) -> Result<(Self, Runtime)>
    where
        F: Fn() -> Result<()> + Send + Sync + 'static,
    {
        let is_cg_v2 = crate::cgroup::is_cgroup_v2()?;
        if is_cg_v2 {
            info!("current host use cgroup v2");
        } else {
            info!("current host use cgroup v1");
        }

        let memcg_initializer: MemcgInitializer = Arc::new(memcg_initializer);
        let mg = MemcgState::new(is_cg_v2, memcg_config, &memcg_initializer)?;

        let comp = compact::Compact::new(is_cg_v2, compact_config)
            .map_err(|e| anyhow!("compact::Compact::new fail: {e}"))?;

        let (cmd_tx, cmd_rx) = mpsc::channel(10);

        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| anyhow!("Builder::new_multi_threa failed: {e}"))?;

        runtime.spawn(async move {
            info!("mem-agent start");
            match mem_agent_loop(cmd_rx, mg, comp, is_cg_v2, memcg_initializer).await {
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
            .map_err(|e| anyhow!("cmd_tx.send cmd failed: {e}"))?;

        let ret = ret_rx
            .await
            .map_err(|e| anyhow!("ret_rx.recv failed: {e}"))?;

        Ok(ret)
    }

    pub async fn memcg_set_config_async(&self, opt: memcg::OptionConfig) -> Result<()> {
        let ret = self
            .send_cmd_async(AgentCmd::MemcgSet(opt))
            .await
            .map_err(|e| anyhow!("send_cmd failed: {e}"))?;

        match ret {
            AgentReturn::Busy => Err(anyhow::Error::new(MemAgentBusy)),
            AgentReturn::Err(e) => Err(anyhow!(
                "mem_agent thread memcg_set_config_async failed: {e}"
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
            .map_err(|e| anyhow!("send_cmd failed: {e}"))?;

        match ret {
            AgentReturn::Err(e) => Err(anyhow!(
                "mem_agent thread compact_set_config_async failed: {e}"
            )),
            AgentReturn::Ok => Ok(()),
            _ => Err(anyhow!(
                "mem_agent thread compact_set_config_async return wrong value"
            )),
        }
    }

    pub async fn memcg_status_async(&self) -> Result<HashMap<String, MemCgroup>> {
        let ret = self
            .send_cmd_async(AgentCmd::MemcgStatus)
            .await
            .map_err(|e| anyhow!("send_cmd failed: {e}"))?;

        let status = match ret {
            AgentReturn::Err(e) => {
                return Err(anyhow!("mem_agent thread memcg_status_async failed: {e}"))
            }
            AgentReturn::Ok => {
                return Err(anyhow!(
                    "mem_agent thread memcg_status_async return wrong value"
                ))
            }
            AgentReturn::Busy => {
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use test_utils::skip_if_not_root;

    #[test]
    fn test_disabled_memcg_does_not_prepare_environment() {
        let mut config = memcg::Config::default();
        config.default.disabled = true;
        let calls = Arc::new(AtomicUsize::new(0));
        let initializer_calls = calls.clone();
        let initializer: MemcgInitializer = Arc::new(move || {
            initializer_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let state = MemcgState::new(false, config, &initializer).unwrap();

        assert!(matches!(state, MemcgState::Disabled(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_dynamic_enable_rolls_back_when_environment_prepare_fails() {
        let mut config = memcg::Config::default();
        config.default.disabled = true;
        let calls = Arc::new(AtomicUsize::new(0));
        let initializer_calls = calls.clone();
        let initializer: MemcgInitializer = Arc::new(move || {
            initializer_calls.fetch_add(1, Ordering::SeqCst);
            Err(anyhow!("debugfs unavailable"))
        });
        let mut state = MemcgState::new(false, config, &initializer).unwrap();
        let option = memcg::OptionConfig {
            default: memcg::SingleOptionConfig {
                disabled: Some(false),
                ..Default::default()
            },
            ..Default::default()
        };

        let error = state
            .set_config(option, false, &initializer)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("prepare memcg environment"));
        assert!(matches!(state, MemcgState::Disabled(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_dynamic_disable_and_reenable_memcg() {
        skip_if_not_root!();
        let mut config = memcg::Config::default();
        config.default.disabled = true;
        let calls = Arc::new(AtomicUsize::new(0));
        let initializer_calls = calls.clone();
        let initializer: MemcgInitializer = Arc::new(move || {
            initializer_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        let mut state = MemcgState::new(false, config, &initializer).unwrap();

        for (disabled, expected_ready, expected_calls) in
            [(false, true, 1), (true, false, 1), (false, true, 2)]
        {
            let option = memcg::OptionConfig {
                default: memcg::SingleOptionConfig {
                    disabled: Some(disabled),
                    ..Default::default()
                },
                ..Default::default()
            };

            state.set_config(option, false, &initializer).await.unwrap();

            assert_eq!(matches!(state, MemcgState::Ready(_)), expected_ready);
            assert_eq!(calls.load(Ordering::SeqCst), expected_calls);
        }
    }

    #[tokio::test]
    async fn test_memcg_busy_error_is_typed() {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(1);
        let agent = MemAgent { cmd_tx };
        tokio::spawn(async move {
            let (_, ret_tx) = cmd_rx.recv().await.unwrap();
            ret_tx.send(AgentReturn::Busy).unwrap();
        });

        let error = agent
            .memcg_set_config_async(memcg::OptionConfig::default())
            .await
            .unwrap_err();

        assert!(error.downcast_ref::<MemAgentBusy>().is_some());
    }

    #[test]
    fn test_dropped_busy_reply_receiver_is_nonfatal() {
        let (ret_tx, ret_rx) = oneshot::channel();
        drop(ret_rx);

        // Regression test: this used to propagate an error out of
        // mem_agent_loop and permanently stop mem-agent command handling.
        send_busy_reply(ret_tx);
    }

    #[tokio::test]
    async fn test_cancelled_reply_does_not_stop_command_loop() {
        let mut config = memcg::Config::default();
        config.default.disabled = true;
        let initializer: MemcgInitializer = Arc::new(|| Ok(()));
        let memcg = MemcgState::new(false, config, &initializer).unwrap();
        let comp = compact::Compact::new(
            false,
            compact::Config {
                disabled: true,
                ..Default::default()
            },
        )
        .unwrap();
        let (cmd_tx, cmd_rx) = mpsc::channel(2);
        let (cancelled_tx, cancelled_rx) = oneshot::channel();
        drop(cancelled_rx);
        cmd_tx
            .send((AgentCmd::MemcgStatus, cancelled_tx))
            .await
            .unwrap();
        let (reply_tx, reply_rx) = oneshot::channel();
        cmd_tx
            .send((AgentCmd::MemcgStatus, reply_tx))
            .await
            .unwrap();

        let task = tokio::spawn(mem_agent_loop(cmd_rx, memcg, comp, false, initializer));
        let reply = tokio::time::timeout(Duration::from_secs(2), reply_rx).await;
        task.abort();
        let _ = task.await;

        assert!(matches!(
            reply.unwrap().unwrap(),
            AgentReturn::MemcgStatus(_)
        ));
    }

    #[test]
    fn test_agent() {
        skip_if_not_root!();
        let mut memcg_config = memcg::Config::default();
        memcg_config.default.disabled = true;
        let compact_config = compact::Config {
            disabled: true,
            ..Default::default()
        };

        let (ma, _rt) = MemAgent::new(memcg_config, compact_config, || Ok(())).unwrap();

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on({
                let mut memcg_config = memcg::OptionConfig::default();
                memcg_config.default.period_secs = Some(120);
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
        skip_if_not_root!();
        let mut memcg_config = memcg::Config::default();
        memcg_config.default.disabled = true;
        let compact_config = compact::Config {
            disabled: true,
            ..Default::default()
        };

        let (ma, _rt) = MemAgent::new(memcg_config, compact_config, || Ok(())).unwrap();

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(ma.memcg_status_async())
            .unwrap();
    }
}
