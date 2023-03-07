# Install Kata Containers on Amazon Web Services

Kata Containers on Amazon Web Services (AWS) makes use of [i3.metal](https://aws.amazon.com/ec2/instance-types/i3/) instances. Most of the installation procedure is identical to that for Kata on your preferred distribution, except that you have to run it on bare metal instances since AWS doesn't support nested virtualization yet. This guide walks you through creating an i3.metal instance.

## Install and Configure AWS CLI

### Requirements

* Python:
  * Python 2 version 2.6.5+
  * Python 3 version 3.3+

### Install

Install with this command:

```bash
$ pip install awscli --upgrade --user
```

### Configure

First, verify it:

```bash
$ aws --version
```

Then configure it:

```bash
$ aws configure
```

Specify the required parameters:

```
AWS Access Key ID []: <your-key-id-from-iam>
AWS Secret Access Key []: <your-secret-access-key-from-iam>
Default region name []: <your-aws-region-for-your-i3-metal-instance>
Default output format [None]: <yaml-or-json-or-empty>
```

Alternatively, you can create the files: `~/.aws/credentials` and `~/.aws/config`:

```bash
$ cat <<EOF > ~/.aws/credentials
[default]
aws_access_key_id = <your-key-id-from-iam>
aws_secret_access_key = <your-secret-access-key-from-iam>
EOF
$ cat <<EOF > ~/.aws/config
[default]
region = <your-aws-region-for-your-i3-metal-instance>
EOF
```

For more information on how to get AWS credentials please refer to [this guide](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html). Alternatively, you can ask the administrator of your AWS account to issue one with the AWS CLI:

```sh
$ aws_username="myusername"
$ aws iam create-access-key --user-name="$aws_username"
```

More general AWS CLI guidelines can be found [here](https://docs.aws.amazon.com/cli/latest/userguide/installing.html).

## Create or Import an EC2 SSH key pair

You will need this to access your instance.

To create:

```bash
$ aws ec2 create-key-pair --key-name MyKeyPair | grep KeyMaterial | cut -d: -f2- | tr -d ' \n\"\,' > MyKeyPair.pem
$ chmod 400 MyKeyPair.pem
```

Alternatively to import using your public SSH key:

```bash
$ aws ec2 import-key-pair --key-name "MyKeyPair" --public-key-material file://MyKeyPair.pub
```

## Launch i3.metal instance

Get the latest Bionic Ubuntu AMI (Amazon Image) or the latest AMI for the Linux distribution you would like to use. For example:

```bash
$ aws ec2 describe-images --owners 099720109477 --filters "Name=name,Values=ubuntu/images/hvm-ssd/ubuntu-bionic-18.04-amd64-server*" --query 'sort_by(Images, &CreationDate)[].ImageId '
```

This command will produce output similar to the following:

```
[
    ...
    "ami-063aa838bd7631e0b",
    "ami-03d5270fcb641f79b"
]
```

Launch the EC2 instance and pick IP the `INSTANCEID`:

```bash
$ aws ec2 run-instances --image-id ami-03d5270fcb641f79b --count 1 --instance-type i3.metal --key-name MyKeyPair --associate-public-ip-address > /tmp/aws.json
$ export INSTANCEID=$(grep InstanceId /tmp/aws.json | cut -d: -f2- | tr -d ' \n\"\,')
```

Wait for the instance to come up, the output of the following command should be `running`:

```bash
$ aws ec2 describe-instances --instance-id=${INSTANCEID} | grep running | cut -d: -f2- | tr -d ' \"\,'
```

Get the public IP address for the instances:

```bash
$ export IP=$(aws ec2 describe-instances --instance-id=${INSTANCEID} | grep PublicIpAddress | cut -d: -f2- | tr -d ' \n\"\,')
```

Refer to [this guide](https://docs.aws.amazon.com/cli/latest/userguide/cli-ec2-launch.html) for more details on how to launch instances with the AWS CLI.

SSH into the machine

```bash
$ ssh -i MyKeyPair.pem ubuntu@${IP}
```

Go onto the next step.

## Install Kata

The process for installing Kata itself on bare metal is identical to that of a virtualization-enabled VM.

For detailed information to install Kata on your distribution of choice, see the [Kata Containers installation user guides](../install/README.md).
