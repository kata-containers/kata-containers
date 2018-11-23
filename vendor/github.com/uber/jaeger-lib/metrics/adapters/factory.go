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

import (
	"github.com/uber/jaeger-lib/metrics"
)

// FactoryWithTags creates metrics with fully qualified name and tags.
type FactoryWithTags interface {
	Counter(name string, tags map[string]string) metrics.Counter
	Gauge(name string, tags map[string]string) metrics.Gauge
	Timer(name string, tags map[string]string) metrics.Timer
}

// Options affect how the adapter factory behaves.
type Options struct {
	ScopeSep string
	TagsSep  string
	TagKVSep string
}

func defaultOptions(options Options) Options {
	o := options
	if o.ScopeSep == "" {
		o.ScopeSep = "."
	}
	if o.TagsSep == "" {
		o.TagsSep = "."
	}
	if o.TagKVSep == "" {
		o.TagKVSep = "_"
	}
	return o
}

// WrapFactoryWithTags creates a real metrics.Factory that supports subscopes.
func WrapFactoryWithTags(f FactoryWithTags, options Options) metrics.Factory {
	return &factory{
		Options: defaultOptions(options),
		factory: f,
		cache:   newCache(),
	}
}

type factory struct {
	Options
	factory FactoryWithTags
	scope   string
	tags    map[string]string
	cache   *cache
}

func (f *factory) Counter(name string, tags map[string]string) metrics.Counter {
	fullName, fullTags, key := f.getKey(name, tags)
	return f.cache.getOrSetCounter(key, func() metrics.Counter {
		return f.factory.Counter(fullName, fullTags)
	})
}

func (f *factory) Gauge(name string, tags map[string]string) metrics.Gauge {
	fullName, fullTags, key := f.getKey(name, tags)
	return f.cache.getOrSetGauge(key, func() metrics.Gauge {
		return f.factory.Gauge(fullName, fullTags)
	})
}

func (f *factory) Timer(name string, tags map[string]string) metrics.Timer {
	fullName, fullTags, key := f.getKey(name, tags)
	return f.cache.getOrSetTimer(key, func() metrics.Timer {
		return f.factory.Timer(fullName, fullTags)
	})
}

func (f *factory) Namespace(name string, tags map[string]string) metrics.Factory {
	return &factory{
		cache:   f.cache,
		scope:   f.subScope(name),
		tags:    f.mergeTags(tags),
		factory: f.factory,
		Options: f.Options,
	}
}

func (f *factory) getKey(name string, tags map[string]string) (fullName string, fullTags map[string]string, key string) {
	fullName = f.subScope(name)
	fullTags = f.mergeTags(tags)
	key = metrics.GetKey(fullName, fullTags, f.TagsSep, f.TagKVSep)
	return
}

func (f *factory) mergeTags(tags map[string]string) map[string]string {
	ret := make(map[string]string, len(f.tags)+len(tags))
	for k, v := range f.tags {
		ret[k] = v
	}
	for k, v := range tags {
		ret[k] = v
	}
	return ret
}

func (f *factory) subScope(name string) string {
	if f.scope == "" {
		return name
	}
	if name == "" {
		return f.scope
	}
	return f.scope + f.ScopeSep + name
}
