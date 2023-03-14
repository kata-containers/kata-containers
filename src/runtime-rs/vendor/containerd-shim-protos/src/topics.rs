/*
   Copyright The containerd Authors.

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

//! Task event topic typically used in shim implementations.

pub const TASK_CREATE_EVENT_TOPIC: &str = "/tasks/create";
pub const TASK_START_EVENT_TOPIC: &str = "/tasks/start";
pub const TASK_OOM_EVENT_TOPIC: &str = "/tasks/oom";
pub const TASK_EXIT_EVENT_TOPIC: &str = "/tasks/exit";
pub const TASK_DELETE_EVENT_TOPIC: &str = "/tasks/delete";
pub const TASK_EXEC_ADDED_EVENT_TOPIC: &str = "/tasks/exec-added";
pub const TASK_EXEC_STARTED_EVENT_TOPIC: &str = "/tasks/exec-started";
pub const TASK_PAUSED_EVENT_TOPIC: &str = "/tasks/paused";
pub const TASK_RESUMED_EVENT_TOPIC: &str = "/tasks/resumed";
pub const TASK_CHECKPOINTED_EVENT_TOPIC: &str = "/tasks/checkpointed";
pub const TASK_UNKNOWN_TOPIC: &str = "/tasks/?";
