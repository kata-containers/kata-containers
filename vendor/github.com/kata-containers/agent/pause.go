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

#define PAUSE_BIN_KEY "pause-bin-key"
#define PAUSE_BIN_VALUE "pause-bin-value"

void __attribute__((constructor)) sandbox_pause() {
	char *value = getenv(PAUSE_BIN_KEY);

	if (value == NULL || strcmp(value, PAUSE_BIN_VALUE)) {
		return;
	}

	for (;;) pause();

	fprintf(stderr, "error: infinite loop terminated\n");
	exit(42);
}
*/
import "C"

const (
	pauseBinKey   = string(C.PAUSE_BIN_KEY)
	pauseBinValue = string(C.PAUSE_BIN_VALUE)
)
