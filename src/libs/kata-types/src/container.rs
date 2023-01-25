// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt::{Display, Formatter};
use std::str::FromStr;

// a container running within a pod
pub(crate) const POD_CONTAINER: &str = "pod_container";
// cri containerd/crio/docker: a container running within a pod
pub(crate) const CONTAINER: &str = "container";

// a pod sandbox container
pub(crate) const POD_SANDBOX: &str = "pod_sandbox";
// cri containerd/crio: a pod sandbox container
pub(crate) const SANDBOX: &str = "sandbox";
// docker: a sandbox sandbox container
pub(crate) const PODSANDBOX: &str = "podsandbox";

pub(crate) const SINGLE_CONTAINER: &str = "single_container";

const STATE_READY: &str = "ready";
const STATE_RUNNING: &str = "running";
const STATE_STOPPED: &str = "stopped";
const STATE_PAUSED: &str = "paused";

/// Error codes for container related operations.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Invalid container type
    #[error("Invalid container type {0}")]
    InvalidContainerType(String),
    /// Invalid container state
    #[error("Invalid sandbox state {0}")]
    InvalidState(String),
    /// Invalid container state transition
    #[error("Can not transit from {0} to {1}")]
    InvalidStateTransition(State, State),
}

/// Types of pod containers: container or sandbox.
#[derive(PartialEq, Debug, Clone)]
pub enum ContainerType {
    /// A pod container.
    PodContainer,
    /// A pod sandbox.
    PodSandbox,
    /// A single container.
    SingleContainer,
}

impl ContainerType {
    /// Check whether it's a pod container.
    pub fn is_pod_container(&self) -> bool {
        matches!(self, ContainerType::PodContainer)
    }

    /// Check whether it's a pod container.
    pub fn is_pod_sandbox(&self) -> bool {
        matches!(self, ContainerType::PodSandbox)
    }
}

impl Display for ContainerType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            ContainerType::PodContainer => write!(f, "{}", POD_CONTAINER),
            ContainerType::PodSandbox => write!(f, "{}", POD_SANDBOX),
            ContainerType::SingleContainer => write!(f, "{}", SINGLE_CONTAINER),
        }
    }
}

impl FromStr for ContainerType {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            POD_CONTAINER | CONTAINER => Ok(ContainerType::PodContainer),
            POD_SANDBOX | PODSANDBOX | SANDBOX => Ok(ContainerType::PodSandbox),
            _ => Err(Error::InvalidContainerType(value.to_owned())),
        }
    }
}

/// Process states.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    /// The container is ready to run.
    Ready,
    /// The container executed the user-specified program but has not exited
    Running,
    /// The container has exited
    Stopped,
    /// The container has been paused.
    Paused,
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            State::Ready => write!(f, "{}", STATE_READY),
            State::Running => write!(f, "{}", STATE_RUNNING),
            State::Stopped => write!(f, "{}", STATE_STOPPED),
            State::Paused => write!(f, "{}", STATE_PAUSED),
        }
    }
}

impl FromStr for State {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            STATE_READY => Ok(State::Ready),
            STATE_RUNNING => Ok(State::Running),
            STATE_STOPPED => Ok(State::Stopped),
            STATE_PAUSED => Ok(State::Paused),
            _ => Err(Error::InvalidState(value.to_owned())),
        }
    }
}

impl State {
    /// Check whether it's a valid state transition from self to the `new_state`.
    pub fn check_transition(self, new_state: State) -> Result<(), Error> {
        match self {
            State::Ready if new_state == State::Running || new_state == State::Stopped => Ok(()),
            State::Running if new_state == State::Stopped => Ok(()),
            State::Stopped if new_state == State::Running => Ok(()),
            State::Paused if new_state == State::Paused => Ok(()),
            _ => Err(Error::InvalidStateTransition(self, new_state)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_type() {
        assert!(ContainerType::PodContainer.is_pod_container());
        assert!(!ContainerType::PodContainer.is_pod_sandbox());

        assert!(ContainerType::PodSandbox.is_pod_sandbox());
        assert!(!ContainerType::PodSandbox.is_pod_container());
    }

    #[test]
    fn test_container_type_display() {
        assert_eq!(format!("{}", ContainerType::PodContainer), POD_CONTAINER);
        assert_eq!(format!("{}", ContainerType::PodSandbox), POD_SANDBOX);
    }

    #[test]
    fn test_container_type_from_str() {
        assert_eq!(
            ContainerType::from_str("pod_container").unwrap(),
            ContainerType::PodContainer
        );
        assert_eq!(
            ContainerType::from_str("container").unwrap(),
            ContainerType::PodContainer
        );
        assert_eq!(
            ContainerType::from_str("pod_sandbox").unwrap(),
            ContainerType::PodSandbox
        );
        assert_eq!(
            ContainerType::from_str("podsandbox").unwrap(),
            ContainerType::PodSandbox
        );
        assert_eq!(
            ContainerType::from_str("sandbox").unwrap(),
            ContainerType::PodSandbox
        );
        ContainerType::from_str("test").unwrap_err();
    }

    #[test]
    fn test_valid() {
        let mut state = State::from_str("invalid_state");
        assert!(state.is_err());

        state = State::from_str("ready");
        assert!(state.is_ok());

        state = State::from_str("running");
        assert!(state.is_ok());

        state = State::from_str("stopped");
        assert!(state.is_ok());
    }

    #[test]
    fn test_valid_transition() {
        use State::*;

        assert!(Ready.check_transition(Ready).is_err());
        assert!(Ready.check_transition(Running).is_ok());
        assert!(Ready.check_transition(Stopped).is_ok());

        assert!(Running.check_transition(Ready).is_err());
        assert!(Running.check_transition(Running).is_err());
        assert!(Running.check_transition(Stopped).is_ok());

        assert!(Stopped.check_transition(Ready).is_err());
        assert!(Stopped.check_transition(Running).is_ok());
        assert!(Stopped.check_transition(Stopped).is_err());
    }
}
