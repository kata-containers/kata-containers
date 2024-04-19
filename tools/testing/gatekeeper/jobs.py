#!/usr/bin/env python3
#
# Copyright (c) 2024 Red Hat Inc.
#
# SPDX-License-Identifier: Apache-2.0

# Keeps checking the current PR until all required jobs pass
# Env variables:
# * REQUIRED_JOBS: comma separated list of required jobs
# * REQUIRED_REGEXPS: comma separated list of regexps for required jobs
# * COMMIT_HASH: Full commit hash we want to be watching
# * GITHUB_REPOSITORY: Github repository (user/repo)

import os
import re
import requests
import time


class Checker:
    def __init__(self):
        required_jobs = os.getenv("REQUIRED_JOBS")
        if required_jobs:
            required_jobs = required_jobs.split(",")
        else:
            required_jobs = []
        required_regexps = os.getenv("REQUIRED_REGEXPS")
        self.required_regexps = []
        # TODO: Add way to specify minimum amount of tests
        # (eg. via \d+: prefix) and check it in status
        if required_regexps:
            for regexp in required_regexps.split(","):
                self.required_regexps.append(re.compile(regexp))
        if not required_jobs and not self.required_regexps:
            raise RuntimeError("No REQUIRED_JOBS or REQUIRED_REGEXPS defined")
        self.results = {job: [] for job in required_jobs}

    def record(self, workflow_id, job):
        """
        Record a job run

        :returns: True on pending job, False on finished jobs
                  (successful or not)
        """
        job_name = job["name"]
        if job_name not in self.results:
            for re_job in self.required_regexps:
                # Required job via regexp
                if re_job.match(job_name):
                    self.results[job_name] = []
                    break
            else:
                # Not a required job
                return False
        if job["status"] != "completed":
            self.results[job_name].append((workflow_id, "Not Completed"))
            return True
        if job["conclusion"] != "success":
            self.results[job_name].append(
                (workflow_id, f"Not success ({job['conclusion']})")
            )
            return False
        self.results[job_name].append((workflow_id, "Passed"))
        return False

    def status(self):
        """
        :returns: 0 - all tests passing; 1 - any failure
        """
        failed = False
        for job, status in self.results.items():
            if not status:
                # Status not reported yet
                return 1
            for stat in status:
                if stat[1] != "Passed":
                    # Status not passed
                    return 1
        if not self.results:
            # No results reported so far
            return 1
        return 0

    def __str__(self):
        """Print status"""
        out = []
        for job, status in self.results.items():
            if not status:
                out.append(f"FAIL: {job} - No results so far")
                continue
            for stat in status:
                if stat[1] == "Passed":
                    out.append(f"PASS: {job} - {stat[0]}")
                else:
                    out.append(f"FAIL: {job} - {stat[0]} - {stat[1]}")
        out = "\n".join(sorted(out))
        if self.status():
            return f"{out}\n\nNot all required jobs passed, check the logs!"
        return f"{out}\n\nAll jobs passed"

    def get_jobs_for_workflow_run(self, run_id):
        """Get jobs from a workflow id"""
        response = requests.get(
            f"https://api.github.com/repos/{os.getenv('GITHUB_REPOSITORY')}/actions/runs/{run_id}/jobs",
            headers={"Accept": "application/vnd.github.v3+json"},
        )
        response.raise_for_status()
        return response.json()["jobs"]

    def check_workflow_runs_status(self):
        """
        Checks if all required jobs passed

        :returns: 0 - all passing; 1 - any failure; 127 some jobs running
        """
        latest_commit_sha = os.getenv("COMMIT_HASH")
        response = requests.get(
            f"https://api.github.com/repos/{os.environ['GITHUB_REPOSITORY']}/actions/runs",
            params={"head_sha": latest_commit_sha},
            headers={"Accept": "application/vnd.github.v3+json"},
        )
        response.raise_for_status()
        workflow_runs = response.json()["workflow_runs"]

        for run in workflow_runs:
            workflow_id = run["id"]
            jobs = self.get_jobs_for_workflow_run(workflow_id)
            for job in jobs:
                if self.record(workflow_id, job):
                    # TODO: Remove this debug output
                    print(f"Some required workflows are still running {job}")
                    return 127
        print(self)
        return self.status()

    def run(self):
        """
        Keep checking the PR until all required jobs finish

        :returns: 0 on success; 1 on failure
        """
        while True:
            ret = self.check_workflow_runs_status()
            if ret == 127:
                time.sleep(60)
                continue
            exit(ret)


if __name__ == "__main__":
    Checker().run()
