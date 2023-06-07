# Kata Containers metrics report generator

The files within this directory can be used to generate a "metrics report"
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
report generation scripts process all subdirectories of `tests/metrics/results` when
generating the report.

> **Note:** By default, the `grabdata.sh` script tries to launch some moderately
> large containers (i.e. 8Gbyte RAM) and may fail to produce some results on a memory
> constrained system.

You can restrict the subset of tests run by `grabdata.sh` via its commandline parameters:

| Option | Description |
| ------ | ----------- |
| -a | Run all tests (default) |
| -d | Run the density tests |
| -h | Print this help |
| -s | Run the storage tests |
| -t | Run the time tests |

## Report generation

Report generation is provided by the `makereport.sh` script. By default this script 
processes all subdirectories of the `tests/metrics/results` directory to generate the report.
To run in the default mode, execute the following:

```sh
$ ./makereport.sh
```

The report generation tool uses [`Rmarkdown`](https://github.com/rstudio/rmarkdown),
[R](https://www.r-project.org/about.html) and [Pandoc](https://pandoc.org/) to produce
a PDF report. To avoid the need for all users to set up a working environment
with all the necessary tooling, the `makereport.sh` script utilises a `Dockerfile` with
the environment pre-defined in order to produce the report. Thus, you need to
have Docker installed on your system in order to run the report generation.

The resulting `metrics_report.pdf` is generated into the `output` subdirectory of the `report`
directory.

## Debugging and development

To aid in script development and debugging, the `makereport.sh` script offers a debug
facility via the `-d` command line option. Using this option will place you into a `bash`
shell within the running `Dockerfile` image used to generate the report, whilst also
mapping your host side `R` scripts from the `report_dockerfile` subdirectory into the
container, thus facilitating a "live" edit/reload/run development cycle.
From there you can examine the Docker image environment, and execute the generation scripts.
E.g., to test the `scaling.R` script, you can execute:

```sh
$ makereport.sh -d
# R
> source('/inputdir/Env.R')
> source('/scripts/lifecycle-time.R')
```

You can then edit the `report_dockerfile/lifecycle-time.R` file on the host, and re-run
the `source('/scripts/lifecycle-time.R')` command inside the still running `R` container
to re-execute the newly edited script.
