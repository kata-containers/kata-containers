# `checkmetrics`

* [Overview](#overview)
    * [JSON file format](#json-file-format)
* [baseline TOML layout](#baseline-toml-layout)
    * [Supported file types](#supported-file-types)
    * [Supported `checktypes`](#supported-checktypes)
* [Options](#options)
    * [TOML base file path (mandatory)](#toml-base-file-path-mandatory)
    * [Debug mode](#debug-mode)
    * [Log file path](#log-file-path)
    * [Metrics results directory path (mandatory)](#metrics-results-directory-path-mandatory)
    * [Percentage presentation mode](#percentage-presentation-mode)
    * [Help](#help)
    * [Version](#version)
* [Output](#output)
    * [Output Columns](#output-columns)
* [Example invocation](#example-invocation)
* [See also](#see-also)

## Overview

The `checkmetrics` tool is used to check the metrics results files in
JSON format.  Results files are checked against configs stored in a
[TOML](https://github.com/toml-lang/toml) file that contains baseline
expectations for the results.

`checkmetrics` checks for a matching results file for each entry in the
TOML file with an appropriate `json` file extension.
Failure to find a matching file is classified as a failure for that
individual TOML entry.

`checkmetrics` continues to process all entries in the TOML file
and prints its final results in a summary table to `stdout`.

`checkmetrics` exits with a failure code if any of the TOML entries
did not complete successfully.

### JSON file format
JSON results files only need to be valid JSON, and contain some form
of numeric results that can be extracted into a string or list of
numeric results using the
[`jq`](https://stedolan.github.io/jq/) JSON query tool.

## baseline TOML layout

The baseline TOML file is composed of one `[[metric]]` section per result
that is processed.  Each section contains a number of parameters, some
optional:

| name          | type     | description                                        |
| ------------- | -------- | -------------------------------------------------- |
| `name`        | `string` | Filename containing results (minus `.json` ext.)   |
| `type`        | `string` | `json`  (optional, `json` is the default)          |
| `description` | `string` | Description of test (optional)                     |
| `checkvar`    | `string` | `jq` query string to extract results from JSON     |
| `checktype`   | `string` | Property to check ("mean", "max" etc.)             |
| `minval`      | `float`  | Minimum value the checked property should be       |
| `maxval`      | `float`  | Maximum value the checked property should be       |
| `midval`      | `float`  | Middle value used for percentage range check       |
| `minpercent`  | `float`  | Minimum percentage from `midval` check boundary    |
| `maxpercent`  | `float`  | Maximum percentage from `midval` check boundary    |

### Supported file types

At this time only `JSON` formatted results files are supported.

### Supported `checktypes`

The following `checktype`s are supported. All are tested to fall within
the bounds set by the `minval` and `maxval`. That is:

> `minval <= Result <= maxval`

| check  | description                                                       |
| ------ | ----------------------------------------------------------------- |
| `mean` | the mean of all the results extracted by the `jq` query           |
| `min`  | the minimum (smallest) result                                     |
| `max`  | the maximum (largest) result                                      |
| `sd`   | the standard deviation of the results                             |
| `cov`  | the coefficient of variation (relative standard deviation)        |

## Options

`checkmetrics` takes a number of options. Some are mandatory.

### TOML base file path (mandatory)

```
--basefile value    path to baseline TOML metrics file
```

### Debug mode

```
--debug             enable debug output in the log
```

### Log file path

```
--log value         set the log file path
```

### Metrics results directory path (mandatory)

```
--metricsdir value  directory containing results files
```

### Percentage presentation mode

```
--percentage        present results as percentage differences
```

> **Note:** Due to the way the internal comparisons are performed, if `minpercent` and
> `maxpercent` are not equal, then the percentage range and midpoint value shown in the
> results table represents the *calculated* midpoint between the min and max, and
> not the supplied `midval`. This makes the results appear skewed, but the
> calculations are correct.

### Help

```
--help, -h          show help
```

### Version

```
--version, -v       print the version
```

## Output

The `checkmetrics` tool outputs a summary table after processing all metrics
sections, and returns a non-zero return code if any of the metrics checks fail.

Example output:

```
Report Summary:
+-----+----------------------+-----------+-----------+-----------+-------+-----------+-----------+------+------+-----+
| P/F |         NAME         |    FLR    |   MEAN    |   CEIL    |  GAP  |    MIN    |    MAX    | RNG  | COV  | ITS |
+-----+----------------------+-----------+-----------+-----------+-------+-----------+-----------+------+------+-----+
| F   | boot-times           |      0.50 |      1.36 |      0.70 | 40.0% |      1.34 |      1.38 | 2.7% | 1.3% |   2 |
| F   | memory-footprint     | 100000.00 | 284570.56 | 110000.00 | 10.0% | 284570.56 | 284570.56 | 0.0% | 0.0% |   1 |
| P   | memory-footprint-ksm | 100000.00 | 101770.22 | 110000.00 | 10.0% | 101770.22 | 101770.22 | 0.0% | 0.0% |   1 |
+-----+----------------------+-----------+-----------+-----------+-------+-----------+-----------+------+------+-----+
Fails: 2, Passes 1
```

Example percentage mode output:

```
Report Summary:
+-----+----------------------+-------+--------+--------+-------+--------+--------+------+------+-----+
| P/F |         NAME         |  FLR  |  MEAN  |  CEIL  |  GAP  |  MIN   |  MAX   | RNG  | COV  | ITS |
+-----+----------------------+-------+--------+--------+-------+--------+--------+------+------+-----+
| *F* | boot-times           | 83.3% | 226.8% | 116.7% | 33.3% | 223.8% | 229.8% | 2.7% | 1.3% |   2 |
| *F* | memory-footprint     | 95.2% | 271.0% | 104.8% | 9.5%  | 271.0% | 271.0% | 0.0% | 0.0% |   1 |
| P   | memory-footprint-ksm | 92.7% | 99.3%  | 107.3% | 14.6% | 99.3%  | 99.3%  | 0.0% | 0.0% |   1 |
+-----+----------------------+-------+--------+--------+-------+--------+--------+------+------+-----+
Fails: 2, Passes 1

```

### Output Columns

| name   | description                                                           |
| ------ | --------------------------------------------------------------------- |
| `P/F`  | Pass/Fail                                                             |
| `NAME` | Name of the test/check                                                |
| `FLR`  | Floor - the `minval` to check against                                 |
| `MEAN` | The mean of the results                                               |
| `CEIL` | Ceiling - the `maxval` to check against                               |
| `GAP`  | The range (gap) between the `minval` and `maxval`, as a % of `minval` |
| `MIN`  | The minimum result in the data set                                    |
| `MAX`  | The maximum result in the data set                                    |
| `RNG`  | The % range (spread) between the min and max result, WRT `min`        |
| `COV`  | The coefficient of variation of the results                           |
| `ITS`  | The number of results (iterations)                                    |

## Example invocation

For example, to invoke the `checkmetrics` tool, enter the following:

```
BASEFILE=`pwd`/../../metrics/baseline/baseline.toml
METRICSDIR=`pwd`/../../metrics/results

$ ./checkmetrics --basefile ${BASEFILE} --metricsdir ${METRICSDIR}
```

## See also

- [CI slaves reference files](ci_slaves)
