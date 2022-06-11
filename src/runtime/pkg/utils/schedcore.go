// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

// PidType is the type of provided pid value and how it should be treated
type PidType int

const (
	pidTypePid            = 0
	pidTypeThreadGroupId  = 1
	pidTypeProcessGroupId = 2

	// Pid affects the current pid
	Pid PidType = pidTypePid
	// ThreadGroup affects all threads in the group
	ThreadGroup PidType = pidTypeThreadGroupId
	// ProcessGroup affects all processes in the group
	ProcessGroup PidType = pidTypeProcessGroupId
)
