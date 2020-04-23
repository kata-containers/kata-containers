//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

/*
#cgo CFLAGS: -Wall
#define _GNU_SOURCE
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PAUSE_BIN "pause-bin"

void __attribute__((constructor)) sandbox_pause(int argc, const char **argv) {
	if (argc != 2 || strcmp(argv[1], PAUSE_BIN)) {
		return;
	}

	for (;;) pause();

	fprintf(stderr, "error: infinite loop terminated\n");
	exit(42);
}
*/
import "C"

const (
	pauseBinArg = string(C.PAUSE_BIN)
)
