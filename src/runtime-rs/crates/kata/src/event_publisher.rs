// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use protobuf::Message;
use slog::{debug, error, info, warn};

use agent_client::Agent;
use shim_proto::events_task::TaskOOM;

const TASK_OOMEVENT_TOPIC: &str = "/tasks/oom";

macro_rules! sl {
    () => {
        slog_scope::logger().new(slog::o!("source" => "event_publisher"))
    };
}

#[derive(Debug, thiserror::Error)]
enum EventError {
    #[error("errors from protobuf: {0}")]
    ProtobufError(#[source] protobuf::ProtobufError),
    #[error("failed to spawn event publisher command: {0}")]
    SpawnChild(#[source] std::io::Error),
    #[error("failed to open stdin")]
    OpenStdin,
    #[error("failed to write stdin: {0}")]
    WriteStdin(#[source] std::io::Error),
    #[error("failed to wait stdout: {0}")]
    WaitStdout(#[source] std::io::Error),
}

struct EventData {
    topic: String,
    data: Vec<u8>,
}

impl EventData {
    fn new(event: impl Message, topic: &str) -> std::result::Result<Self, EventError> {
        let any_message = ::protobuf::well_known_types::Any {
            type_url: event.descriptor().full_name().into(),
            value: event.write_to_bytes().map_err(EventError::ProtobufError)?,
            ..Default::default()
        };
        let data = any_message
            .write_to_bytes()
            .map_err(EventError::ProtobufError)?;

        Ok(Self {
            topic: topic.into(),
            data,
        })
    }

    fn new_oom(container_id: &str) -> std::result::Result<Self, EventError> {
        let event = TaskOOM {
            container_id: container_id.into(),
            ..Default::default()
        };
        Self::new(event, TASK_OOMEVENT_TOPIC)
    }
}

/// Event publisher to send events to the Containerd.
///
/// It will automatically forward all TaskOOM events from agent to Containerd.
/// TODO: support grace shutdown.
pub struct EventPublisher {
    containerd_binary: String,
    address: String,
    namespace: String,
}

impl EventPublisher {
    /// Create a new instance of ['EventPublisher'].
    pub fn new(containerd_binary: &str, address: &str, namespace: &str) -> Self {
        Self {
            containerd_binary: containerd_binary.into(),
            address: address.into(),
            namespace: namespace.into(),
        }
    }

    /// Start to forward events from agent
    pub fn start(&self, agent: Arc<dyn Agent>) {
        let (sender, receiver) = channel();
        self.run_forwarder(receiver);
        self.start_oom_watcher(agent, sender);
    }

    fn run_forwarder(&self, receiver: Receiver<EventData>) {
        let builder = std::thread::Builder::new().name("event_publisher".to_string());
        let containerd_binary = self.containerd_binary.clone();
        let address = self.address.clone();
        let namespace = self.namespace.clone();

        let _ = builder.spawn(move || {
            loop {
                match receiver.recv() {
                    Ok(event) => {
                        if let Err(err) =
                            Self::send_event(&containerd_binary, &address, &namespace, &event)
                        {
                            error!(
                                sl!(),
                                "failed to send event type {} error {:?}", event.topic, err
                            );
                        }
                    }
                    Err(err) => {
                        warn!(
                            sl!(),
                            "failed to receive event for event forwarder error {:?}", err
                        );
                        break;
                    }
                }
            }
            info!(sl!(), "stop run event publisher");
        });
    }

    fn start_oom_watcher(&self, agent: Arc<dyn Agent>, sender: Sender<EventData>) {
        let event_builder = std::thread::Builder::new().name("oom_event_watcher".to_string());
        let _ = event_builder.spawn(move || loop {
            match agent.get_oom_event() {
                Ok(container_id) => match EventData::new_oom(&container_id) {
                    Err(e) => {
                        error!(sl!(), "failed to generate event_data with error: {:?}", e);
                    }
                    Ok(event_data) => {
                        if let Err(e) = sender.send(event_data) {
                            error!(sl!(), "failed to send event_data with error: {:?}", e);
                        }
                    }
                },
                Err(err) => {
                    warn!(sl!(), "failed to get oom event error {:?}", err);
                    break;
                }
            }
        });
    }

    fn send_event(
        containerd_binary: &str,
        address: &str,
        namespace: &str,
        event: &EventData,
    ) -> std::result::Result<(), EventError> {
        let mut child = Command::new(containerd_binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(&[
                "--address",
                address,
                "publish",
                "--topic",
                &event.topic,
                "--namespace",
                namespace,
            ])
            .spawn()
            .map_err(EventError::SpawnChild)?;

        let stdin = child.stdin.as_mut().ok_or(EventError::OpenStdin)?;
        stdin
            .write_all(event.data.as_slice())
            .map_err(EventError::WriteStdin)?;
        let output = child.wait_with_output().map_err(EventError::WaitStdout)?;
        debug!(sl!(), "get output: {:?}", output);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oom_event() {
        let container_id = "id1";

        let ev = EventData::new_oom(container_id).unwrap();
        assert_eq!(ev.topic, TASK_OOMEVENT_TOPIC);

        let any_message: protobuf::well_known_types::Any =
            protobuf::parse_from_bytes(ev.data.as_slice()).unwrap();
        assert_eq!(any_message.get_type_url(), "containerd.events.TaskOOM");

        let oom_event: TaskOOM = protobuf::parse_from_bytes(any_message.get_value()).unwrap();
        assert_eq!(oom_event.get_container_id(), container_id);
    }
}
