#!/usr/bin/env python3
# -*- coding: utf-8 -*-
#
# Copyright (c) 2023 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
#
from nv_attestation_sdk import attestation
import os 
import json

client = attestation.Attestation()
client.set_name("localhost")
print ("[LOCAL] node name :", client.get_name())
file = "NVGPULocalPolicyExample.json"

client.add_verifier(attestation.Devices.GPU, attestation.Environment.LOCAL, "", "")
with open(os.path.join(os.path.dirname(__file__), file)) as json_file:
    json_data = json.load(json_file)
    att_result_policy = json.dumps(json_data)

client.get_verifiers()

print ("[LOCAL] call attest() - expecting True")
client.attest()

print ("[LOCAL] token : "+str(client.get_token()))
print ("[LOCAL] call validate_token() - expecting True")
client.validate_token(att_result_policy)

