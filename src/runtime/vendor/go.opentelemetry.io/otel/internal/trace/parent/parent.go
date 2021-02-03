// Copyright The OpenTelemetry Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package parent

import (
	"context"

	"go.opentelemetry.io/otel/label"
	"go.opentelemetry.io/otel/trace"
)

func GetSpanContextAndLinks(ctx context.Context, ignoreContext bool) (trace.SpanContext, bool, []trace.Link) {
	lsctx := trace.SpanContextFromContext(ctx)
	rsctx := trace.RemoteSpanContextFromContext(ctx)

	if ignoreContext {
		links := addLinkIfValid(nil, lsctx, "current")
		links = addLinkIfValid(links, rsctx, "remote")

		return trace.SpanContext{}, false, links
	}
	if lsctx.IsValid() {
		return lsctx, false, nil
	}
	if rsctx.IsValid() {
		return rsctx, true, nil
	}
	return trace.SpanContext{}, false, nil
}

func addLinkIfValid(links []trace.Link, sc trace.SpanContext, kind string) []trace.Link {
	if !sc.IsValid() {
		return links
	}
	return append(links, trace.Link{
		SpanContext: sc,
		Attributes: []label.KeyValue{
			label.String("ignored-on-demand", kind),
		},
	})
}
