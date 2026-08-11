// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Context, Result};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zvariant::{OwnedObjectPath, OwnedValue};

use super::dbus::{Connection, Method};

const SYSTEMD_SOCKET: &str = "/run/systemd/private";
const SYSTEMD_SERVICE: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";
const UNIT_INTERFACE: &str = "org.freedesktop.systemd1.Unit";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
const UNIT_MODE_REPLACE: &str = "replace";
const JOB_REMOVED_SIGNAL: &str = "JobRemoved";

/// Cap on a single read from, or write to, systemd. Everything we ask it for
/// is answered from memory, so the only thing this can cut short is a systemd
/// that has stopped answering us altogether.
const IO_TIMEOUT: Duration = Duration::from_secs(60);

const JOB_WAIT_TIMEOUT: Duration = Duration::from_secs(300);

/// The body of a JobRemoved signal: job id, job path, unit name, and the
/// result systemd retired the job with.
type JobRemoved = (u32, OwnedObjectPath, String, String);

pub async fn systemctl(args: &[&str]) -> Result<()> {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();

    // Our D-Bus client is deliberately blocking, so it runs where blocking is
    // allowed rather than on a runtime worker.
    tokio::task::spawn_blocking(move || run(&args))
        .await
        .context("The systemd worker thread panicked")?
}

/// When systemd last saw `unit` enter the active state, or `None` if it never
/// did in this boot.
///
/// Readable by a process other than the one that asked for the restart, which
/// is the point: a container torn down mid-restart leaves no record of its own.
pub async fn unit_active_since(unit: &str) -> Result<Option<SystemTime>> {
    let unit = unit.to_owned();

    tokio::task::spawn_blocking(move || {
        let mut connection = Connection::connect(SYSTEMD_SOCKET, IO_TIMEOUT)
            .context("Failed to connect to the host systemd private socket")?;

        active_enter_timestamp(&mut connection, &service_name(&unit))
    })
    .await
    .context("The systemd worker thread panicked")?
}

fn manager(member: &str) -> Method<'_> {
    Method {
        destination: SYSTEMD_SERVICE,
        path: MANAGER_PATH,
        interface: MANAGER_INTERFACE,
        member,
    }
}

fn service_name(name: &str) -> String {
    if name.contains('.') {
        name.to_owned()
    } else {
        format!("{name}.service")
    }
}

fn run(args: &[String]) -> Result<()> {
    let mut connection = Connection::connect(SYSTEMD_SOCKET, IO_TIMEOUT)
        .context("Failed to connect to the host systemd private socket")?;

    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["daemon-reload"] => reload(&mut connection)?,
        ["restart", unit] => {
            let unit = service_name(unit);
            run_job(&mut connection, "RestartUnit", &unit)?;

            // A job that completes says the unit was started, not that it
            // stayed up: a service that dies on us right afterwards is still
            // a failed restart as far as the caller is concerned.
            if active_state(&mut connection, &unit)? == "failed" {
                bail!("Systemd unit {unit} failed to restart");
            }
        }
        ["stop", unit] => {
            let unit = service_name(unit);
            run_job(&mut connection, "StopUnit", &unit)?;
        }
        ["enable", unit] => {
            let unit = service_name(unit);
            enable_unit(&mut connection, &unit)?;
        }
        ["disable", unit] => {
            let unit = service_name(unit);
            disable_unit(&mut connection, &unit)?;
        }
        ["disable", "--now", unit] => {
            let unit = service_name(unit);
            run_job(&mut connection, "StopUnit", &unit)?;
            disable_unit(&mut connection, &unit)?;
        }
        ["is-active", "--quiet", unit] => {
            let unit = service_name(unit);
            if active_state(&mut connection, &unit)? != "active" {
                bail!("Systemd unit {unit} is not active");
            }
        }
        _ => bail!("Unsupported systemctl arguments: {}", args.join(" ")),
    }

    Ok(())
}

fn reload(connection: &mut Connection) -> Result<()> {
    // systemd defers the Reload() reply until the reload completes, and sends
    // it through the bus object it tears down while reloading. Over the
    // private socket that reply is simply lost, so waiting for it would hang.
    // PID 1 reloads synchronously, so any later call is still ordered after it.
    connection
        .call_no_reply(&manager("Reload"), &())
        .context("Failed to reload systemd")
}

fn enable_unit(connection: &mut Connection, unit: &str) -> Result<()> {
    connection
        .call(&manager("EnableUnitFiles"), &(vec![unit], false, false))
        .with_context(|| format!("Failed to enable systemd unit {unit}"))?;

    Ok(())
}

fn disable_unit(connection: &mut Connection, unit: &str) -> Result<()> {
    connection
        .call(&manager("DisableUnitFiles"), &(vec![unit], false))
        .with_context(|| format!("Failed to disable systemd unit {unit}"))?;

    Ok(())
}

/// Run a job on `unit` and wait for how it turned out.
///
/// StopUnit/RestartUnit only enqueue a job and hand back its path, whereas
/// `systemctl` waits for the job to finish and fails when it did not succeed.
/// Callers rely on both halves of that: the stage restarting the node's CRI
/// has to outlive the restart, otherwise the CRI loses track of the caller's
/// own container and reports its exit status as unknown.
fn run_job(connection: &mut Connection, method: &str, unit: &str) -> Result<()> {
    // Subscribing first is what makes the result observable at all: systemd
    // only emits signals to a peer that asked for them, and a job started
    // before we asked could finish before we are listening.
    connection
        .call(&manager("Subscribe"), &())
        .context("Failed to subscribe to systemd job notifications")?;

    let reply = connection
        .call(&manager(method), &(unit, UNIT_MODE_REPLACE))
        .with_context(|| format!("Failed to run {method} on systemd unit {unit}"))?;
    let job: OwnedObjectPath = reply
        .body()
        .with_context(|| format!("Failed to read the job {method} started on {unit}"))?;

    wait_for_job(connection, &job, unit)
}

/// Block until systemd retires `job`, and report what it made of it.
///
/// JobRemoved carries the one thing a job's disappearance alone does not say:
/// whether it ran to completion, or was cancelled, timed out, or failed along
/// with a dependency.
fn wait_for_job(connection: &mut Connection, job: &OwnedObjectPath, unit: &str) -> Result<()> {
    let deadline = Instant::now() + JOB_WAIT_TIMEOUT;

    loop {
        let signal = connection
            .wait_for_signal(JOB_REMOVED_SIGNAL, deadline)
            .with_context(|| format!("Failed to wait for the systemd job on {unit}"))?;

        let (_, removed, _, result): JobRemoved = signal
            .body()
            .context("Failed to read a systemd JobRemoved signal")?;
        if removed != *job {
            continue;
        }

        // "skipped" is systemd's way of saying the job had nothing left to do,
        // such as stopping a unit that is already stopped.
        return match result.as_str() {
            "done" | "skipped" => Ok(()),
            other => bail!("Systemd job on {unit} did not succeed: {other}"),
        };
    }
}

fn unit_property(connection: &mut Connection, unit: &str, name: &str) -> Result<OwnedValue> {
    let reply = connection
        .call(&manager("GetUnit"), &(unit,))
        .with_context(|| format!("Failed to find systemd unit {unit}"))?;
    let path: OwnedObjectPath = reply
        .body()
        .with_context(|| format!("Failed to read the object path of systemd unit {unit}"))?;

    let property = Method {
        destination: SYSTEMD_SERVICE,
        path: path.as_str(),
        interface: PROPERTIES_INTERFACE,
        member: "Get",
    };
    let reply = connection
        .call(&property, &(UNIT_INTERFACE, name))
        .with_context(|| format!("Failed to get {name} of systemd unit {unit}"))?;

    reply
        .body()
        .with_context(|| format!("Failed to read {name} of systemd unit {unit}"))
}

fn active_state(connection: &mut Connection, unit: &str) -> Result<String> {
    let state = unit_property(connection, unit, "ActiveState")?;

    String::try_from(state)
        .with_context(|| format!("Systemd reported a non-string state for unit {unit}"))
}

fn active_enter_timestamp(connection: &mut Connection, unit: &str) -> Result<Option<SystemTime>> {
    let timestamp = unit_property(connection, unit, "ActiveEnterTimestamp")?;

    // Microseconds on the same clock as a file's mtime; zero for a unit that
    // has never been active.
    let micros = u64::try_from(timestamp)
        .with_context(|| format!("Systemd reported a non-numeric timestamp for unit {unit}"))?;
    if micros == 0 {
        return Ok(None);
    }

    Ok(Some(UNIX_EPOCH + Duration::from_micros(micros)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_names_get_a_service_suffix() {
        assert_eq!(service_name("containerd"), "containerd.service");
        assert_eq!(service_name("containerd.service"), "containerd.service");
        assert_eq!(service_name("kata.socket"), "kata.socket");
    }
}
