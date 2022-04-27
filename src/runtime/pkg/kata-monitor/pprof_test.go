// Copyright (c) 2022 Red Hat Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"bytes"
	"net/http"
	"net/url"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestCopyResponseAddingSandboxIdToHref(t *testing.T) {
	assert := assert.New(t)

	htmlIn := strings.NewReader(`
<html>
<head>
<title>/debug/pprof/</title>
<style>
.profile-name{
	display:inline-block;
	width:6rem;
}
</style>
</head>
<body>
/debug/pprof/<br>
<br>
Types of profiles available:
<table>
<thead><td>Count</td><td>Profile</td></thead>
<tr><td>27</td><td><a href='allocs?debug=1'>allocs</a></td></tr>
<tr><td>0</td><td><a href='block?debug=1'>block</a></td></tr>
<tr><td>0</td><td><a href='cmdline?debug=1'>cmdline</a></td></tr>
<tr><td>39</td><td><a href='goroutine?debug=1'>goroutine</a></td></tr>
<tr><td>27</td><td><a href='heap?debug=1'>heap</a></td></tr>
<tr><td>0</td><td><a href='mutex?debug=1'>mutex</a></td></tr>
<tr><td>0</td><td><a href='profile?debug=1'>profile</a></td></tr>
<tr><td>10</td><td><a href='threadcreate?debug=1'>threadcreate</a></td></tr>
<tr><td>0</td><td><a href='trace?debug=1'>trace</a></td></tr>
</table>
<a href="goroutine?debug=2">full goroutine stack dump</a>
<br>
<p>
Profile Descriptions:
<ul>
<li><div class=profile-name>allocs: </div> A sampling of all past memory allocations</li>
<li><div class=profile-name>block: </div> Stack traces that led to blocking on synchronization primitives</li>
<li><div class=profile-name>cmdline: </div> The command line invocation of the current program</li>
<li><div class=profile-name>goroutine: </div> Stack traces of all current goroutines</li>
<li><div class=profile-name>heap: </div> A sampling of memory allocations of live objects. You can specify the gc GET parameter to run GC before taking the heap sample.</li>
<li><div class=profile-name>mutex: </div> Stack traces of holders of contended mutexes</li>
<li><div class=profile-name>profile: </div> CPU profile. You can specify the duration in the seconds GET parameter. After you get the profile file, use the go tool pprof command to investigate the profile.</li>
<li><div class=profile-name>threadcreate: </div> Stack traces that led to the creation of new OS threads</li>
<li><div class=profile-name>trace: </div> A trace of execution of the current program. You can specify the duration in the seconds GET parameter. After you get the trace file, use the go tool trace command to investigate the trace.</li>
</ul>
</p>
</body>
</html>`)

	htmlExpected := bytes.NewBufferString(`
<html>
<head>
<title>/debug/pprof/</title>
<style>
.profile-name{
	display:inline-block;
	width:6rem;
}
</style>
</head>
<body>
/debug/pprof/<br>
<br>
Types of profiles available:
<table>
<thead><td>Count</td><td>Profile</td></thead>
<tr><td>27</td><td><a href='allocs?sandbox=1234567890&debug=1'>allocs</a></td></tr>
<tr><td>0</td><td><a href='block?sandbox=1234567890&debug=1'>block</a></td></tr>
<tr><td>0</td><td><a href='cmdline?sandbox=1234567890&debug=1'>cmdline</a></td></tr>
<tr><td>39</td><td><a href='goroutine?sandbox=1234567890&debug=1'>goroutine</a></td></tr>
<tr><td>27</td><td><a href='heap?sandbox=1234567890&debug=1'>heap</a></td></tr>
<tr><td>0</td><td><a href='mutex?sandbox=1234567890&debug=1'>mutex</a></td></tr>
<tr><td>0</td><td><a href='profile?sandbox=1234567890&debug=1'>profile</a></td></tr>
<tr><td>10</td><td><a href='threadcreate?sandbox=1234567890&debug=1'>threadcreate</a></td></tr>
<tr><td>0</td><td><a href='trace?sandbox=1234567890&debug=1'>trace</a></td></tr>
</table>
<a href="goroutine?sandbox=1234567890&debug=2">full goroutine stack dump</a>
<br>
<p>
Profile Descriptions:
<ul>
<li><div class=profile-name>allocs: </div> A sampling of all past memory allocations</li>
<li><div class=profile-name>block: </div> Stack traces that led to blocking on synchronization primitives</li>
<li><div class=profile-name>cmdline: </div> The command line invocation of the current program</li>
<li><div class=profile-name>goroutine: </div> Stack traces of all current goroutines</li>
<li><div class=profile-name>heap: </div> A sampling of memory allocations of live objects. You can specify the gc GET parameter to run GC before taking the heap sample.</li>
<li><div class=profile-name>mutex: </div> Stack traces of holders of contended mutexes</li>
<li><div class=profile-name>profile: </div> CPU profile. You can specify the duration in the seconds GET parameter. After you get the profile file, use the go tool pprof command to investigate the profile.</li>
<li><div class=profile-name>threadcreate: </div> Stack traces that led to the creation of new OS threads</li>
<li><div class=profile-name>trace: </div> A trace of execution of the current program. You can specify the duration in the seconds GET parameter. After you get the trace file, use the go tool trace command to investigate the trace.</li>
</ul>
</p>
</body>
</html>`)

	req := &http.Request{URL: &url.URL{RawQuery: "sandbox=1234567890"}}
	buf := bytes.NewBuffer(nil)
	copyResponseAddingSandboxIdToHref(req, buf, htmlIn)
	assert.Equal(htmlExpected, buf)
}
