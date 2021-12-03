// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// message receiver buffer size
const MESSAGE_RECEIVER_BUFFER_SIZE: usize = 1;

#[derive(Debug)]
pub enum Action {
    Start,
    Stop,
    Shutdown,
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
