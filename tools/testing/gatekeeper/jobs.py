#!/usr/bin/env python3
#
# Copyright (c) 2024 Red Hat Inc.
#
# SPDX-License-Identifier: Apache-2.0

"""
Keeps checking the current PR until all required jobs pass
Env variables:
* REQUIRED_JOBS: comma separated list of required jobs (in form of
                 "$workflow / $job")
* REQUIRED_REGEXPS: comma separated list of regexps for required jobs
* COMMIT_HASH: Full commit hash we want to be watching
* GITHUB_REPOSITORY: Github repository (user/repo)
Sample execution (GH token can be excluded):
GITHUB_TOKEN="..." REQUIRED_JOBS="skipper / skipper"
REQUIRED_REGEXPS=".*"
COMMIT_HASH=b8382cea886ad9a8f77d237bcfc0eba0c98775dd
GITHUB_REPOSITORY=kata-containers/kata-containers
python3 jobs.py
"""

import os
import re
import sys
import time
import requests


PASS = 0
FAIL = 1
RUNNING = 127


_GH_HEADERS = {"Accept": "application/vnd.github.v3+json"}
if os.environ.get("GITHUB_TOKEN"):
    _GH_HEADERS["Authorization"] = f"token {os.environ['GITHUB_TOKEN']}"
_GH_RUNS_URL = ("https://api.github.com/repos/"
                f"{os.environ['GITHUB_REPOSITORY']}/actions/runs")


class Checker:
    """Object to keep watching required GH action workflows"""
    def __init__(self):
        self.latest_commit_sha = os.getenv("COMMIT_HASH")
        required_jobs = os.getenv("REQUIRED_JOBS")
        if required_jobs:
            required_jobs = required_jobs.split(";")
        else:
            required_jobs = []
        required_regexps = os.getenv("REQUIRED_REGEXPS")
        self.required_regexps = []
        # TODO: Add way to specify minimum amount of tests
        # (eg. via \d+: prefix) and check it in status
        if required_regexps:
            for regexp in required_regexps.split(";"):
                self.required_regexps.append(re.compile(regexp))
        if not required_jobs and not self.required_regexps:
            raise RuntimeError("No REQUIRED_JOBS or REQUIRED_REGEXPS defined")
        # Set all required jobs as RUNNING to enforce waiting for them
        self.results = {job: RUNNING for job in required_jobs}

    def record(self, workflow, job):
        """
        Records a job run
        """
        job_name = f"{workflow} / {job['name']}"
        if job_name not in self.results:
            for re_job in self.required_regexps:
                # Required job via regexp
                if re_job.match(job_name):
                    break
            else:
                # Not a required job
                return
        if job["status"] != "completed":
            self.results[job_name] = RUNNING
            return
        if job["conclusion"] != "success":
            self.results[job_name] = job['conclusion']
            return
        self.results[job_name] = PASS

    def status(self):
        """
        :returns: 0 - all tests passing; 127 - no failures but some
            tests in progress; 1 - any failure
        """
        running = False
        if not self.results:
            # No results reported so far
            return FAIL
        for status in self.results.values():
            if status == RUNNING:
                running |= True
            elif status != PASS:
                # Status not passed
                return FAIL
        if running:
            return RUNNING
        return PASS

    def __str__(self):
        """Sumarize the current status"""
        good = []
        bad = []
        warn = []
        for job, status in self.results.items():
            if status == RUNNING:
                warn.append(f"WARN: {job} - Still running")
            elif status == PASS:
                good.append(f"PASS: {job} - success")
            else:
                bad.append(f"FAIL: {job} - Not passed - {status}")
        out = '\n'.join(sorted(good) + sorted(warn) + sorted(bad))
        stat = self.status()
        if stat == RUNNING:
            status = "Some jobs are still running."
        elif stat == PASS:
            status = "All required jobs passed"
        elif not self.results:
            status = ("No required jobs for regexps: " +
                      ";".join([_.pattern for _ in self.required_regexps]))
        else:
            status = "Not all required jobs passed!"
        return f"{out}\n\n{status}"

    def get_jobs_for_workflow_run(self, run_id):
        """Get jobs from a workflow id"""
        total_count = -1
        jobs = []
        page = 1
        while True:
            response = requests.get(
                f"{_GH_RUNS_URL}/{run_id}/jobs?per_page=100&page={page}",
                headers=_GH_HEADERS,
                timeout=60
            )
            response.raise_for_status()
            output = response.json()
            jobs.extend(output["jobs"])
            total_count = max(total_count, output["total_count"])
            if len(jobs) >= total_count:
                break
            page += 1
        return jobs

    def check_workflow_runs_status(self):
        """
        Checks if all required jobs passed

        :returns: 0 - all passing; 1 - any failure; 127 some jobs running
        """
        # TODO: Check if we need pagination here as well
        response = requests.get(
            _GH_RUNS_URL,
            params={"head_sha": self.latest_commit_sha},
            headers=_GH_HEADERS,
            timeout=60
        )
        response.raise_for_status()
        workflow_runs = response.json()["workflow_runs"]

        for run in workflow_runs:
            jobs = self.get_jobs_for_workflow_run(run["id"])
            for job in jobs:
                self.record(run["name"], job)
        print(self)
        return self.status()

    def run(self):
        """
        Keep checking the PR until all required jobs finish

        :returns: 0 on success; 1 on failure
        """
        print(f"Gatekeeper for project={os.environ['GITHUB_REPOSITORY']} and "
              f"SHA={self.latest_commit_sha}")
        while True:
            ret = self.check_workflow_runs_status()
            if ret == RUNNING:
                running_jobs = len([job
                                    for job, status in self.results.items()
                                    if status == RUNNING])
                print(f"{running_jobs} jobs are still running...")
                time.sleep(180)
                continue
            sys.exit(ret)


if __name__ == "__main__":
    Checker().run()
