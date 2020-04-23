// Copyright (c) 2018 Uber Technologies, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package adapters

import "github.com/uber/jaeger-lib/metrics"

// FactoryWithoutTags creates metrics based on name only, without tags.
// Suitable for integrating with statsd-like backends that don't support tags.
type FactoryWithoutTags interface {
	Counter(name string) metrics.Counter
	Gauge(name string) metrics.Gauge
	Timer(name string) metrics.Timer
}

// WrapFactoryWithoutTags creates a real metrics.Factory that supports subscopes.
func WrapFactoryWithoutTags(f FactoryWithoutTags, options Options) metrics.Factory {
	return WrapFactoryWithTags(
		&tagless{
			Options: defaultOptions(options),
			factory: f,
		},
		options,
	)
}

// tagless implements FactoryWithTags
type tagless struct {
	Options
	factory FactoryWithoutTags
}

func (f *tagless) Counter(name string, tags map[string]string) metrics.Counter {
	fullName := f.getFullName(name, tags)
	return f.factory.Counter(fullName)
}

func (f *tagless) Gauge(name string, tags map[string]string) metrics.Gauge {
	fullName := f.getFullName(name, tags)
	return f.factory.Gauge(fullName)
}

func (f *tagless) Timer(name string, tags map[string]string) metrics.Timer {
	fullName := f.getFullName(name, tags)
	return f.factory.Timer(fullName)
}

func (f *tagless) getFullName(name string, tags map[string]string) string {
	return metrics.GetKey(name, tags, f.TagsSep, f.TagKVSep)
}
