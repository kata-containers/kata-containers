#
# Copyright (c) 2020 Ant Financial
#
# SPDX-License-Identifier: Apache-2.0
#
# This file is formatted by https://github.com/google/yapf
#

import sys
import requests
import json


def underbar_to_camel(s):
    ss = s.split("_")
    r = ""
    for s in ss:
        r = r + s.capitalize()
    return r


def json_tag(s):
    return "`json:\"" + s + "\"`"


metric_declare_stmt = """{fn} = prometheus.NewGaugeVec(prometheus.GaugeOpts{{
        Namespace: fcMetricsNS,
        Name:      "{tag}",
        Help:      "{help}",
    }},
        []string{{"item"}},
    )"""


def lowercase(s):
    # RTCDeviceMetrics need special rule
    if s == "RTCDeviceMetrics":
        return "rtcDeviceMetrics"
    return s[0].lower() + s[1:]


def uppercase(s):
    # RTCDeviceMetrics need special rule
    if s == "rtcDeviceMetrics":
        return "RTCDeviceMetrics"
    return s[0].upper() + s[1:]


def generate_declaration(fn, tag, help):
    fn = lowercase(fn)
    return metric_declare_stmt.format(fn=fn, tag=tag, help=help)


type_name_tag_map = {}

# only for FirecrackerMetrics struct
type_name_field_name_map = {}
# all metrics list
all_metrics = []
# all lines that used to definition go struct.
go_types = []


def parse_metric_declaration(lines):
    # help: name: tag: items:{go_var: json_tag}
    metric = {}
    in_struct = False
    in_FirecrackerMetrics = False
    line_count = len(lines)
    i = 0
    while i < line_count:
        line = lines[i]
        i = i + 1

        line = line.rstrip()
        lline = line.lstrip()
        if lline.find("#[") > -1:
            continue
        if line == "struct SerializeToUtcTimestampMs;":
            # code blocks that impl some struct, but not struct definition, skip
            go_types.pop()
            continue

        # skip impl Serialize for SerializeToUtcTimestampMs
        if lline.find("impl ") == 0:
            # code blocks that impl some struct, but not struct definition, skip
            j = i
            # find then end of the blcok and skip
            while j < line_count:
                tmp_line = lines[j]
                j = j + 1
                if tmp_line == "}":
                    i = j + 1
                    break
            continue

        if lline.find("///") > -1:
            if in_struct == False:
                in_struct = True
                hp = line[3:].strip()
                metric = {"help": hp}
            go_types.append(line)
            continue
        if line == "}":
            in_struct = False
            in_FirecrackerMetrics = False
            all_metrics.append(metric)
            go_types.append(line)
            continue

        ss = line.split()
        if len(ss) > 1 and ss[0] == "//":
            go_types.append(line)
            continue
        elif len(ss) == 4:
            ## struct definition
            ## pub struct MmdsMetrics {
            ## or type FirecrackerMetrics struct {
            type_name = ss[2]
            if ss[0] == "type":
                type_name = ss[1]
            if type_name == "FirecrackerMetrics":
                # FirecrackerMetrics is a special struct that contains all detailed structs.
                in_FirecrackerMetrics = True

            metric["name"] = lowercase(type_name)

            go_types.append("type {} struct {{".format(type_name))

        elif len(ss) == 3:
            ## properties definition
            ## pub rx_accepted: SharedMetric,
            # mapped to metric's item label
            ## or pub patch_api_requests: PatchRequestsMetrics,
            data_type_value = ss[1].rstrip(":")
            data_type = ss[2].rstrip(",")
            if data_type == "SharedMetric":
                data_type = "uint64"
            else:
                type_name_tag_map[data_type] = data_type_value
            if in_FirecrackerMetrics:
                type_name_field_name_map[data_type] = underbar_to_camel(
                    data_type_value)

            go_types.append("   {} {} {}".format(
                underbar_to_camel(data_type_value), data_type,
                json_tag(data_type_value)))

            items = metric.get("items", [])
            items.append((underbar_to_camel(data_type_value), data_type_value))
            metric["items"] = items
        elif len(ss) > 1 and ss[0] == "utc_timestamp_ms:":
            continue
        else:
            go_types.append(line)


# declare metrics value statements
declaration_stmt = []
# register metrics value statements
register_stmt = []
# set metrics value statements
set_metrics_stmt = []
set_metrics_stmt_tpl = "{metric_var}.WithLabelValues(\"{item}\").Set(float64({instance_var}.{field}))"


def get_tag_var_name(type_name):
    return type_name_tag_map.get(uppercase(type_name), "FIXME")


def generate_metric_declaration_codes():
    for m in all_metrics:
        metric_var_name = m["name"]
        if metric_var_name == "firecrackerMetrics":
            # the outer wrapper struct
            continue
        tn = get_tag_var_name(metric_var_name)
        ds = generate_declaration(metric_var_name, tn, m["help"])
        declaration_stmt.append(ds)
        register_stmt.append("    prometheus.MustRegister(" + metric_var_name +
                             ")")

        # set metrics
        set_metrics_stmt.append("")
        set_metrics_stmt.append("// set metrics for " + metric_var_name)
        for i in m.get("items", []):
            iv = "fm." + uppercase(
                type_name_field_name_map.get(uppercase(metric_var_name),
                                             "FIXME"))
            set_stmt = set_metrics_stmt_tpl.format(metric_var=metric_var_name,
                                                   item=i[1],
                                                   instance_var=iv,
                                                   field=i[0])
            set_metrics_stmt.append(set_stmt)


def print_metric_declaration():

    print "var ("
    for x in declaration_stmt:
        print x
        print
    print ")"

    print
    print

    print "func registerFirecrackerMetrics() {"
    for x in register_stmt:
        print x
    print "}"

    print
    print

    print "func updateFirecrackerMetrics(fm *FirecrackerMetrics) {"
    for x in set_metrics_stmt:
        print x
    print "}"

    print ""
    print "// golang types from rust version"
    print ""
    for x in go_types:
        print x


def download(url):
    ## url="https://github.com/firecracker-microvm/firecracker/blob/b417a783e3e3dce60da9c2e745ffaf35595fc0be/src/logger/src/metrics.rs#L255-L687"
    ss = url.split("#")
    u = ss[0]
    lines = ss[1]
    (start, end) = lines.replace("L", "").split("-")

    ## convert to https://raw.githubusercontent.com/firecracker-microvm/firecracker/b417a783e3e3dce60da9c2e745ffaf35595fc0be/src/logger/src/metrics.rs
    u = u.replace("github.com",
                  "raw.githubusercontent.com").replace("firecracker/blob",
                                                       "firecracker")
    r = requests.get(u)
    start = int(start)
    end = int(end)
    lines = r.content.splitlines()[(start - 1):end]
    return lines


header_tpl = """
// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//
// WARNING: This file is auto-generated - DO NOT EDIT!

package virtcontainers

import (
	"github.com/prometheus/client_golang/prometheus"
)

const fcMetricsNS = "kata_firecracker"

"""


def print_header():
    print(header_tpl)


if __name__ == '__main__':

    if len(sys.argv) != 2:
        print("use: python fc_metrics.py <url>")
        exit(1)

    url = sys.argv[1]
    lines = download(url)

    print_header()

    # parse/generate/print metrics declaration codes
    parse_metric_declaration(lines)
    generate_metric_declaration_codes()
    print_metric_declaration()
