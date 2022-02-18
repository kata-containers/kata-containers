/*
 * Copyright (c) 2020. Ant Group. All rights reserved.
 *
 * SPDX-License-Identifier: Apache-2.0
 */

// copy from https://github.com/containerd/nydus-snapshotter/blob/38b23bcd0658f2bd9d99083d320b727bf73129b7/pkg/utils/retry/retry.go
package retry

import (
	"fmt"
	"math/rand"
	"strings"
	"time"
)

type RetryableFunc func() error

var (
	DefaultAttempts      = uint(10)
	DefaultDelayMS       = 100 * time.Millisecond
	DefaultMaxJitterMS   = 100 * time.Millisecond
	DefaultOnRetry       = func(n uint, err error) {}
	DefaultRetryIf       = IsRecoverable
	DefaultDelayType     = CombineDelay(BackOffDelay, RandomDelay)
	DefaultLastErrorOnly = false
)

// Function signature of retry if function
type RetryIfFunc func(error) bool

// Function signature of OnRetry function
// n = count of attempts
type OnRetryFunc func(n uint, err error)

type DelayTypeFunc func(n uint, config *Config) time.Duration

type Config struct {
	onRetry       OnRetryFunc
	retryIf       RetryIfFunc
	delayType     DelayTypeFunc
	delay         time.Duration
	maxDelay      time.Duration
	maxJitter     time.Duration
	attempts      uint
	lastErrorOnly bool
}

// Option represents an option for retry.
type Option func(*Config)

// return the direct last error that came from the retried function
// default is false (return wrapped errors with everything)
func LastErrorOnly(lastErrorOnly bool) Option {
	return func(c *Config) {
		c.lastErrorOnly = lastErrorOnly
	}
}

// Attempts set count of retry
// default is 10
func Attempts(attempts uint) Option {
	return func(c *Config) {
		c.attempts = attempts
	}
}

// Delay set delay between retry
// default is 100ms
func Delay(delay time.Duration) Option {
	return func(c *Config) {
		c.delay = delay
	}
}

// MaxDelay set maximum delay between retry
// does not apply by default
func MaxDelay(maxDelay time.Duration) Option {
	return func(c *Config) {
		c.maxDelay = maxDelay
	}
}

// MaxJitter sets the maximum random Jitter between retries for RandomDelay
func MaxJitter(maxJitter time.Duration) Option {
	return func(c *Config) {
		c.maxJitter = maxJitter
	}
}

// DelayType set type of the delay between retries
// default is BackOff
func DelayType(delayType DelayTypeFunc) Option {
	return func(c *Config) {
		c.delayType = delayType
	}
}

// BackOffDelay is a DelayType which increases delay between consecutive retries
func BackOffDelay(n uint, config *Config) time.Duration {
	return config.delay * (1 << n)
}

// FixedDelay is a DelayType which keeps delay the same through all iterations
func FixedDelay(_ uint, config *Config) time.Duration {
	return config.delay
}

// RandomDelay is a DelayType which picks a random delay up to config.maxJitter
func RandomDelay(_ uint, config *Config) time.Duration {
	return time.Duration(rand.Int63n(int64(config.maxJitter)))
}

// CombineDelay is a DelayType the combines all of the specified delays into a new DelayTypeFunc
func CombineDelay(delays ...DelayTypeFunc) DelayTypeFunc {
	return func(n uint, config *Config) time.Duration {
		var total time.Duration
		for _, delay := range delays {
			total += delay(n, config)
		}
		return total
	}
}

// OnRetry function callback are called each retry
//
// log each retry example:
//
//	retry.Do(
//		func() error {
//			return errors.New("some error")
//		},
//		retry.OnRetry(func(n uint, err error) {
//			log.Printf("#%d: %s\n", n, err)
//		}),
//	)
func OnRetry(onRetry OnRetryFunc) Option {
	return func(c *Config) {
		c.onRetry = onRetry
	}
}

// RetryIf controls whether a retry should be attempted after an error
// (assuming there are any retry attempts remaining)
//
// skip retry if special error example:
//
//	retry.Do(
//		func() error {
//			return errors.New("special error")
//		},
//		retry.RetryIf(func(err error) bool {
//			if err.Error() == "special error" {
//				return false
//			}
//			return true
//		})
//	)
//
// By default RetryIf stops execution if the error is wrapped using `retry.Unrecoverable`,
// so above example may also be shortened to:
//
//	retry.Do(
//		func() error {
//			return retry.Unrecoverable(errors.New("special error"))
//		}
//	)
func RetryIf(retryIf RetryIfFunc) Option {
	return func(c *Config) {
		c.retryIf = retryIf
	}
}

func Do(retryableFunc RetryableFunc, opts ...Option) error {
	var n uint

	// default
	config := &Config{
		attempts:      DefaultAttempts,
		delay:         DefaultDelayMS,
		maxJitter:     DefaultMaxJitterMS,
		onRetry:       DefaultOnRetry,
		retryIf:       DefaultRetryIf,
		delayType:     DefaultDelayType,
		lastErrorOnly: DefaultLastErrorOnly,
	}

	// apply opts
	for _, opt := range opts {
		opt(config)
	}

	var errorLog Error
	if !config.lastErrorOnly {
		errorLog = make(Error, config.attempts)
	} else {
		errorLog = make(Error, 1)
	}

	lastErrIndex := n
	for n < config.attempts {
		err := retryableFunc()

		if err != nil {
			errorLog[lastErrIndex] = unpackUnrecoverable(err)

			if !config.retryIf(err) {
				break
			}

			config.onRetry(n, err)

			// if this is last attempt - don't wait
			if n == config.attempts-1 {
				break
			}

			delayTime := config.delayType(n, config)
			if config.maxDelay > 0 && delayTime > config.maxDelay {
				delayTime = config.maxDelay
			}
			time.Sleep(delayTime)
		} else {
			return nil
		}

		n++
		if !config.lastErrorOnly {
			lastErrIndex = n
		}
	}

	if config.lastErrorOnly {
		return errorLog[lastErrIndex]
	}
	return errorLog
}

// Error type represents list of errors in retry
type Error []error

// Error method return string representation of Error
// It is an implementation of error interface
func (e Error) Error() string {
	logWithNumber := make([]string, lenWithoutNil(e))
	for i, l := range e {
		if l != nil {
			logWithNumber[i] = fmt.Sprintf("#%d: %s", i+1, l.Error())
		}
	}

	return fmt.Sprintf("All attempts fail:\n%s", strings.Join(logWithNumber, "\n"))
}

func lenWithoutNil(e Error) (count int) {
	for _, v := range e {
		if v != nil {
			count++
		}
	}

	return
}

// WrappedErrors returns the list of errors that this Error is wrapping.
// It is an implementation of the `errwrap.Wrapper` interface
// in package [errwrap](https://github.com/hashicorp/errwrap) so that
// `retry.Error` can be used with that library.
func (e Error) WrappedErrors() []error {
	return e
}

type unrecoverableError struct {
	error
}

// Unrecoverable wraps an error in `unrecoverableError` struct
func Unrecoverable(err error) error {
	return unrecoverableError{err}
}

// IsRecoverable checks if error is an instance of `unrecoverableError`
func IsRecoverable(err error) bool {
	_, isUnrecoverable := err.(unrecoverableError)
	return !isUnrecoverable
}

func unpackUnrecoverable(err error) error {
	if unrecoverable, isUnrecoverable := err.(unrecoverableError); isUnrecoverable {
		return unrecoverable.error
	}

	return err
}
