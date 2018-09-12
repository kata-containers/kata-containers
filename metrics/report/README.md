* [Kata Containers metrics report generator](#kata-containers-metrics-report-generator)
   * [Data gathering](#data-gathering)
   * [Report generation](#report-generation)

# Kata Containers metrics report generator

The files within this directory can be used to generate a 'metrics report'
for Kata Containers.

The primary workflow consists of two stages:

1) Run the provided report metrics data gathering scripts on the system(s) you wish
to analyze.
2) Run the provided report generation script to analyze the data and generate a
report file.

## Data gathering

Data gathering is provided by the `grabdata.sh` script. When run, this script
executes a set of tests from the `tests/metrics` directory. The JSON results files
will be placed into the `tests/metrics/results` directory.

Once the results are generated, create a suitably named subdirectory of
`tests/metrics/results`, and move the JSON files into it.

Repeat this process if you want to compare multiple sets of results. Note, the
report generation scripts process all subfolders of `tests/metrics/results` when
generating the report.

> **Note:** By default, the `grabdata.sh` script tries to launch some moderately
> large containers (i.e. 8Gbyte RAM) and may fail to produce some results on a memory
> constrained system.

## Report generation

Report generation is provided by the `makereport.sh` script. By default this script 
processes all subdirectories of the `tests/metrics/results` directory to generate the report.
To run in the default mode, execute the following:

```sh
$ ./makereport.sh
```

The report generation tool uses [Rmarkdown](https://github.com/rstudio/rmarkdown),
[R](https://www.r-project.org/about.html) and [pandoc](https://pandoc.org/) to produce
a PDF report. To avoid the need for all users to set up a working environment
with all the necessary tooling, the `makereport.sh` script utilises a `Dockerfile` with
the environment pre-defined in order to produce the report. Thus, you need to
have Docker installed on your system in order to run the report generation.

The resulting `metrics_report.pdf` is generated into the `output` subdir of the `report`
directory.
