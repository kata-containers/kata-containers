// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
package main

import (
	"encoding/csv"
	"encoding/json"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
	"time"

	env "github.com/kata-containers/tests/metrics/env"
	exec "github.com/kata-containers/tests/metrics/exec"
	"github.com/kata-containers/tests/metrics/k8s"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

var log = logrus.New()

var (
	optContainerRuntime = "container-runtime"
	optDebug            = "debug"
	optOutputDir        = "output-dir"
	optTestName         = "test-name"
	// fio options
	optFioBlockSize = "fio.block-size"
	optFioDirect    = "fio.direct"
	optFioIoDepth   = "fio.iodepth"
	optFioSize      = "fio.size"
	optFioNumJobs   = "fio.numjobs"
)

type RwFioOp struct {
	BandwidthKb int     `json:"bw"`
	IOPS        float64 `json:"iops"`
}

type fioResult struct {
	GlobalOptions struct {
		IOEngine string `json:"ioengine"`
		RW       string `json:"rw"`
	} `json:"global options"`
	Jobs []struct {
		JobName string  `json:"jobname"`
		Read    RwFioOp `json:"read"`
		Write   RwFioOp `json:"write"`
	} `json:"jobs"`
}

// Run fio in k8s metrics test in K8s
func (c fioTestConfig) run() (result fioResult, err error) {
	log.Infof("Running fio config: %s", c.jobFile)

	pod := k8s.Pod{YamlPath: c.k8sYaml}

	log.Infof("Delete pod if already created")
	err = pod.Delete()
	if err != nil {
		return result, err
	}

	log.Infof("Create pod: %s", pod.YamlPath)
	err = pod.Run()
	if err != nil {
		return result, err
	}

	defer func() {
		log.Info("Deleting pod")
		delErr := pod.Delete()
		if delErr != nil {
			log.Error(delErr)
			if err != nil {
				err = errors.Wrapf(err, "Could not delete pod after: %s", delErr)
			}
		}
	}()

	destDir := "/home/fio-jobs"
	_, err = pod.Exec("mkdir " + destDir)
	if err != nil {
		return result, err
	}

	dstJobFile := path.Join(destDir, "jobFile")
	err = pod.CopyFromHost(c.jobFile, dstJobFile)
	if err != nil {
		return result, err
	}

	_, err = pod.Exec("apt update")
	if err != nil {
		return result, err
	}
	_, err = pod.Exec("apt install -y fio")
	if err != nil {
		return result, err
	}

	err = env.DropCaches()
	if err != nil {
		return result, err
	}

	var directStr string
	if c.direct {
		directStr = "1"
	} else {
		directStr = "0"
	}

	cmdFio := "fio"
	cmdFio += " --append-terse "
	cmdFio += " --blocksize=" + c.blocksize
	cmdFio += " --direct=" + directStr
	cmdFio += " --directory=" + c.directory
	cmdFio += " --iodepth=" + c.iodepth
	cmdFio += " --numjobs=" + c.numjobs
	cmdFio += " --runtime=" + c.runtime
	cmdFio += " --size=" + c.size
	cmdFio += " --output-format=json"
	cmdFio += " " + dstJobFile

	log.Infof("Exec fio")
	output, err := pod.Exec(cmdFio, k8s.ExecOptShowStdOut())
	if err != nil {
		return result, err
	}
	err = json.Unmarshal([]byte(output), &result)
	if err != nil {
		return result, errors.Wrapf(err, "failed to unmarshall output : %s", output)
	}

	log.Infof("ioengine:%s", result.GlobalOptions.IOEngine)
	log.Infof("rw:%s", result.GlobalOptions.RW)
	if len(result.Jobs) == 0 {
		return result, errors.New("No jobs found after parsing fio results")
	}

	testDir := path.Join(c.outputDir, filepath.Base(c.jobFile))
	err = os.MkdirAll(testDir, 0775)
	if err != nil {
		return result, errors.Wrapf(err, "failed to create test directory for :%s", c.jobFile)
	}
	outputFile := path.Join(testDir, "output.json")
	log.Infof("Store results output in : %s", outputFile)

	err = os.WriteFile(outputFile, []byte(output), 0644)
	if err != nil {
		return result, err
	}

	return result, nil
}

type fioTestConfig struct {
	//test options
	k8sYaml          string
	containerRuntime string
	outputDir        string

	//fio options
	blocksize string
	directory string
	iodepth   string
	numjobs   string
	jobFile   string
	loops     string
	runtime   string
	size      string

	direct bool
}

func runFioJobs(testDirPath string, cfg fioTestConfig) (results []fioResult, err error) {
	fioJobsDir, err := filepath.Abs(path.Join(testDirPath, "fio-jobs"))
	if err != nil {
		return results, err
	}

	files, err := os.ReadDir(fioJobsDir)
	if err != nil {
		log.Fatal(err)
		return results, err
	}

	if cfg.containerRuntime == "" {
		return results, errors.New("containerRuntime is empty")
	}

	podYAMLName := cfg.containerRuntime + ".yaml"
	cfg.k8sYaml = path.Join(testDirPath, podYAMLName)

	if len(files) == 0 {
		return results, errors.New("No fio configs found")
	}

	for _, file := range files {
		cfg.jobFile = path.Join(fioJobsDir, file.Name())
		r, err := cfg.run()
		if err != nil {
			return results, err
		}
		results = append(results, r)

		log.Infof("workload:%s", r.Jobs[0].JobName)
		log.Infof("bw_r:%d", r.Jobs[0].Read.BandwidthKb)
		log.Infof("IOPS_r:%f", r.Jobs[0].Read.IOPS)
		log.Infof("bw_w:%d", r.Jobs[0].Write.BandwidthKb)
		log.Infof("IOPS_w:%f", r.Jobs[0].Write.IOPS)

		waitTime := 5
		log.Debugf("Sleep %d seconds(if not wait sometimes create another pod timesout)", waitTime)
		time.Sleep(time.Duration(waitTime) * time.Second)
	}
	return results, err

}

func generateResultsView(testName string, results []fioResult, outputDir string) error {
	outputFile := path.Join(outputDir, "results.csv")
	f, err := os.Create(outputFile)
	if err != nil {
		return err
	}
	defer f.Close()

	log.Infof("Creating results output in %s", outputFile)

	w := csv.NewWriter(f)

	headers := []string{"NAME", "WORKLOAD", "bw_r", "bw_w", "IOPS_r", "IOPS_w"}
	err = w.Write(headers)
	if err != nil {
		return err
	}

	for _, r := range results {
		if len(r.Jobs) == 0 {
			return errors.Errorf("fio result has no jobs: %v", r)
		}
		row := []string{testName}
		row = append(row, r.Jobs[0].JobName)
		row = append(row, fmt.Sprintf("%d", r.Jobs[0].Read.BandwidthKb))
		row = append(row, fmt.Sprintf("%d", r.Jobs[0].Write.BandwidthKb))
		row = append(row, fmt.Sprintf("%f", r.Jobs[0].Read.IOPS))
		row = append(row, fmt.Sprintf("%f", r.Jobs[0].Write.IOPS))
		if err := w.Write(row); err != nil {
			return err
		}
	}

	w.Flush()

	return w.Error()
}

func main() {

	app := &cli.App{
		Flags: []cli.Flag{
			&cli.BoolFlag{
				Name:  optDebug,
				Usage: "Logs in debug level",
			},
			&cli.StringFlag{
				Name:  optTestName,
				Value: "kata-fio-test",
				Usage: "Change the fio test name for reports",
			},
			&cli.StringFlag{
				Name:  optOutputDir,
				Value: ".",
				Usage: "Use a file to store results",
			},
			&cli.StringFlag{
				Name:  optContainerRuntime,
				Value: "kata",
				Usage: "Choose the runtime to use",
			},
			//fio options
			&cli.StringFlag{
				Name:  optFioSize,
				Value: "200M",
				Usage: "File size to use for tests",
			},
			&cli.StringFlag{
				Name:  optFioBlockSize,
				Value: "4K",
				Usage: "Block size for fio tests",
			},
			&cli.BoolFlag{
				Name:  optFioDirect,
				Usage: "Use direct io",
			},
			&cli.StringFlag{
				Name:  optFioIoDepth,
				Value: "16",
				Usage: "Number of I/O units to keep in flight against the file",
			},
			&cli.StringFlag{
				Name:  optFioNumJobs,
				Value: "1",
				Usage: "Number of clones (processes/threads performing the same workload) of this job",
			},
		},
		Action: func(c *cli.Context) error {
			jobsDir := c.Args().First()

			if jobsDir == "" {
				cli.SubcommandHelpTemplate = strings.Replace(cli.SubcommandHelpTemplate, "[arguments...]", "<test-config-dir>", -1)
				cli.ShowCommandHelp(c, "")
				return errors.New("Missing <test-config-dir>")
			}

			if c.Bool(optDebug) {
				log.SetLevel(logrus.DebugLevel)
				k8s.Debug = true
				env.Debug = true
			}

			exec.SetLogger(log)
			k8s.SetLogger(log)
			env.SetLogger(log)

			testName := c.String(optTestName)

			outputDir, err := filepath.Abs(path.Join(c.String(optOutputDir), testName))
			if err != nil {
				return err
			}

			cfg := fioTestConfig{
				blocksize:        c.String(optFioBlockSize),
				direct:           c.Bool(optFioDirect),
				directory:        ".",
				iodepth:          c.String(optFioIoDepth),
				loops:            "3",
				numjobs:          c.String(optFioNumJobs),
				runtime:          "20",
				size:             c.String(optFioSize),
				containerRuntime: c.String(optContainerRuntime),
				outputDir:        outputDir,
			}

			log.Infof("Results will be created in %s", cfg.outputDir)

			err = os.MkdirAll(cfg.outputDir, 0775)
			if err != nil {
				return err
			}

			results, err := runFioJobs(jobsDir, cfg)
			if err != nil {
				return err
			}

			return generateResultsView(c.String(optTestName), results, outputDir)
		},
	}

	err := app.Run(os.Args)
	if err != nil {
		log.Fatal(err)
	}

}
