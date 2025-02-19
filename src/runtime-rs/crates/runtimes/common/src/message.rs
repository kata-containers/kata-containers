// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::sync::Arc;

use anyhow::{Context, Result};
use containerd_shim_protos::events::task::{TaskExit, TaskOOM};
use containerd_shim_protos::protobuf::Message as ProtobufMessage;
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// message receiver buffer size
const MESSAGE_RECEIVER_BUFFER_SIZE: usize = 1;

#[derive(Debug)]
pub enum Action {
    Start,
    Stop,
    Shutdown,
    Event(Arc<dyn Event + Send + Sync>),
}

#[derive(Debug)]
pub struct Message {
    pub action: Action,
    pub resp_sender: Option<Sender<Result<()>>>,
}

impl Message {
    pub fn new(action: Action) -> Self {
        Message {
            action,
            resp_sender: None,
        }
    }

    pub fn new_with_receiver(action: Action) -> (Receiver<Result<()>>, Self) {
        let (resp_sender, receiver) = channel(MESSAGE_RECEIVER_BUFFER_SIZE);
        (
            receiver,
            Message {
                action,
                resp_sender: Some(resp_sender),
            },
        )
    }
}

const TASK_OOM_EVENT_TOPIC: &str = "/tasks/oom";
const TASK_EXIT_EVENT_TOPIC: &str = "/tasks/exit";

const TASK_OOM_EVENT_URL: &str = "containerd.events.TaskOOM";
const TASK_EXIT_EVENT_URL: &str = "containerd.events.TaskExit";

pub trait Event: std::fmt::Debug + Send {
    fn r#type(&self) -> String;
    fn type_url(&self) -> String;
    fn value(&self) -> Result<Vec<u8>>;
}

impl Event for TaskOOM {
    fn r#type(&self) -> String {
        TASK_OOM_EVENT_TOPIC.to_string()
    }

    fn type_url(&self) -> String {
        TASK_OOM_EVENT_URL.to_string()
    }

    fn value(&self) -> Result<Vec<u8>> {
        self.write_to_bytes().context("get oom value")
    }
}

impl Event for TaskExit {
    fn r#type(&self) -> String {
        TASK_EXIT_EVENT_TOPIC.to_string()
    }

    fn type_url(&self) -> String {
        TASK_EXIT_EVENT_URL.to_string()
    }

    fn value(&self) -> Result<Vec<u8>> {
        self.write_to_bytes().context("get exit value")
    }
}
