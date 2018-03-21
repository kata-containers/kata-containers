# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# This Dockerfile contains all the dependencies that
# the Python application requires, including Python itself.

# Usage: FROM [image name]
FROM python:3.4-alpine

# Add the current directory . into the /code in the image
ADD . /code

# Set the working directory to /code
WORKDIR /code

# Install the Python dependencies
RUN pip install -r requirements.txt

CMD ["python", "app.py"]
