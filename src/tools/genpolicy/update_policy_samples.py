import os
import subprocess
import sys
import json

# runs genpolicy tools on the following files
# should run this after any change to genpolicy
# usage: python3 update_policy_samples.py

samples = ""

with open('policy_samples.json') as f:
    samples = json.load(f)

defaultYamls = samples["default"]

silently_ignored = samples["silently_ignored"]

no_policy = samples["no_policy"]

file_base_path = "../../agent/samples/policy/yaml"

def runCmd(arg):
    proc = subprocess.run([arg], stdout=sys.stdout, stderr=sys.stderr, universal_newlines=True, input="", shell=True)
    print(f"COMMAND: {arg}")
    if proc.returncode != 0:
        print(f"`{arg}` failed with exit code {proc.returncode}. Stderr: {proc.stderr}, Stdout: {proc.stdout}")
    return proc

# check we can access all files we are about to update
for file in defaultYamls + silently_ignored + no_policy:
    filepath = os.path.join(file_base_path, file)
    if not os.path.exists(filepath):
        print(f"filepath does not exists: {filepath}")

# build tool
runCmd("cargo build")

# update files
genpolicy_path = "target/debug/genpolicy"
for file in defaultYamls:
    runCmd(f"{genpolicy_path} -y {os.path.join(file_base_path, file)}")

for file in silently_ignored:
    runCmd(f"{genpolicy_path} -y {os.path.join(file_base_path, file)} -s")

for file in no_policy:
    runCmd(f"{genpolicy_path} -y {os.path.join(file_base_path, file)}")

