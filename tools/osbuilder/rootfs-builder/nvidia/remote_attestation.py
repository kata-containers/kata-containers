#!/usr/bin/env python3
# -*- coding: utf-8 -*-
#
# Copyright (c) 2023 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
#
from nv_attestation_sdk import attestation
import os 
import json
import requests


def nras_reachable():
	# 10 Retries each second before giving up
	for x in range(10):
		print("Checking if [NRAS] is reachable with retry count: " + str(x)) 
		r = requests.get("https://rim.attestation.nvidia.com/v1/rim/ids")
		data = r.content
		print("[NRAS] request response: " + str(data))
		if r.status_code == 200:
			return True
		
		print("[NRAS] request error: " + str(r.status_code))

	return False

if nras_reachable():
	print ("[NRAS] is reachable")
else:
	print ("[NRAS] is not reachable")
	exit(1)



NRAS_URL="https://nras.attestation.nvidia.com/v1/attest/gpu"
client = attestation.Attestation()
client.set_name("localhost")
client.set_nonce("931d8dd0add203ac3d8b4fbde75e115278eefcdceac5b87671a748f32364dfcb")
print ("[NRAS] node name :", client.get_name())

client.add_verifier(attestation.Devices.GPU, attestation.Environment.REMOTE, NRAS_URL, "")

file = "NVGPURemotePolicyExample.json"

with open(os.path.join(os.path.dirname(__file__), file)) as json_file:
    json_data = json.load(json_file)
    remote_att_result_policy = json.dumps(json_data)

print(client.get_verifiers())

print ("[NRAS] call attest() - expecting True")
print(client.attest())

print ("[NRAS] token : "+str(client.get_token()))

print ("[NRAS] call validate_token() - expecting True")
print(client.validate_token(remote_att_result_policy))
