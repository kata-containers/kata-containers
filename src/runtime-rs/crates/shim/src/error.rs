// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to parse argument {0}")]
    ParseArgument(String),
    #[error("failed to get bundle path")]
    GetBundlePath,
    #[error("invalid argument")]
    InvalidArgument,
    #[error("argument is empty {0}")]
    ArgumentIsEmpty(String),
    #[error("invalid path {0}")]
    InvalidPath(String),

    // File
    #[error("failed to open file {0}")]
    FileOpen(String),
    #[error("failed to get file metadata {0}")]
    FileGetMetadata(String),
    #[error("failed to read file {0}")]
    FileRead(String),
    #[error("failed to write file {0}")]
    FileWrite(String),

    #[error("empty sandbox id")]
    EmptySandboxId,
    #[error("failed to get self exec: {0}")]
    SelfExec(#[source] std::io::Error),
    #[error("failed to bind socket at {1} with error: {0}")]
    BindSocket(#[source] std::io::Error, PathBuf),
    #[error("failed to spawn child: {0}")]
    SpawnChild(#[source] std::io::Error),
    #[error("failed to clean container {0}")]
    CleanUpContainer(String),
    #[error("failed to get env variable: {0}")]
    EnvVar(#[source] std::env::VarError),
    #[error("failed to parse server fd environment variable {0}")]
    ServerFd(String),
    #[error("failed to wait ttrpc server when {0}")]
    WaitServer(String),
    #[error("failed to get system time: {0}")]
    SystemTime(#[source] std::time::SystemTimeError),
    #[error("failed to parse pid")]
    ParsePid,
}
