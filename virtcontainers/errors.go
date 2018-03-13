//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

import (
	"errors"
)

// common error objects used for argument checking
var (
	errNeedPod         = errors.New("Pod must be specified")
	errNeedPodID       = errors.New("Pod ID cannot be empty")
	errNeedContainerID = errors.New("Container ID cannot be empty")
	errNeedFile        = errors.New("File cannot be empty")
	errNeedState       = errors.New("State cannot be empty")
	errInvalidResource = errors.New("Invalid pod resource")
)
