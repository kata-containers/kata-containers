# Install Kata Containers on Google Compute Engine

* [Create an Image with Nested Virtualization Enabled](#create-an-image-with-nested-virtualization-enabled)
    * [Create the Image](#create-the-image)
    * [Verify VMX is Available](#verify-vmx-is-available)
* [Install Kata](#install-kata)
* [Create a Kata-enabled Image](#create-a-kata-enabled-image)

Kata Containers on Google Compute Engine (GCE) makes use of [nested virtualization](https://cloud.google.com/compute/docs/instances/enable-nested-virtualization-vm-instances). Most of the installation procedure is identical to that for Kata on your preferred distribution, but enabling nested virtualization currently requires extra steps on GCE. This guide walks you through creating an image and instance with nested virtualization enabled. Note that `kata-runtime check` checks for nested virtualization, but does not fail if support is not found.

As a pre-requisite this guide assumes an installed and configured instance of the [Google Cloud SDK](https://cloud.google.com/sdk/downloads). For a zero-configuration option, all of the commands below were been tested under [Google Cloud Shell](https://cloud.google.com/shell/) (as of Jun 2018). Verify your `gcloud` installation and configuration:

```bash
$ gcloud info || { echo "ERROR: no Google Cloud SDK"; exit 1; }
```

## Create an Image with Nested Virtualization Enabled

VM images on GCE are grouped into families under projects. Officially supported images are automatically discoverable with `gcloud compute images list`. That command produces a list similar to the following (likely with different image names):

```bash
$ gcloud compute images list
NAME                                                  PROJECT            FAMILY                            DEPRECATED  STATUS
centos-7-v20180523                                    centos-cloud       centos-7                                      READY
coreos-stable-1745-5-0-v20180531                      coreos-cloud       coreos-stable                                 READY
cos-beta-67-10575-45-0                                cos-cloud          cos-beta                                      READY
cos-stable-66-10452-89-0                              cos-cloud          cos-stable                                    READY
debian-9-stretch-v20180510                            debian-cloud       debian-9                                      READY
rhel-7-v20180522                                      rhel-cloud         rhel-7                                        READY
sles-11-sp4-v20180523                                 suse-cloud         sles-11                                       READY
ubuntu-1604-xenial-v20180522                          ubuntu-os-cloud    ubuntu-1604-lts                               READY
ubuntu-1804-bionic-v20180522                          ubuntu-os-cloud    ubuntu-1804-lts                               READY
```

Each distribution has its own project, and each project can host images for multiple versions of the distribution, typically grouped into families. We recommend you select images by project and family, rather than by name. This ensures any scripts or other automation always works with a non-deprecated image, including security updates, updates to GCE-specific scripts, etc.

### Create the Image

The following example (substitute your preferred distribution project and image family) produces an image with nested virtualization enabled in your currently active GCE project:

```bash
$ SOURCE_IMAGE_PROJECT=ubuntu-os-cloud
$ SOURCE_IMAGE_FAMILY=ubuntu-1804-lts
$ IMAGE_NAME=${SOURCE_IMAGE_FAMILY}-nested

$ gcloud compute images create \
    --source-image-project $SOURCE_IMAGE_PROJECT \
    --source-image-family $SOURCE_IMAGE_FAMILY \
    --licenses=https://www.googleapis.com/compute/v1/projects/vm-options/global/licenses/enable-vmx \
    $IMAGE_NAME
```

If successful, `gcloud` reports that the image was created. Verify that the image has the nested virtualization license with `gcloud compute images describe $IMAGE_NAME`. This produces output like the following (some fields have been removed for clarity and to redact personal info):

```yaml
diskSizeGb: '10'
kind: compute#image
licenseCodes:
  - '1002001'
  - '5926592092274602096'
licenses:
  - https://www.googleapis.com/compute/v1/projects/vm-options/global/licenses/enable-vmx
  - https://www.googleapis.com/compute/v1/projects/ubuntu-os-cloud/global/licenses/ubuntu-1804-lts
name: ubuntu-1804-lts-nested
sourceImage: https://www.googleapis.com/compute/v1/projects/ubuntu-os-cloud/global/images/ubuntu-1804-bionic-v20180522
sourceImageId: '3280575157699667619'
sourceType: RAW
status: READY
```

The primary criterion of interest here is the presence of the `enable-vmx` license. Without that licence Kata will not work. Without that license Kata does not work. The presence of that license instructs the Google Compute Engine hypervisor to enable Intel's VT-x instructions in virtual machines created from the image. Note that nested virtualization is only available in VMs running on Intel Haswell or later CPU micro-architectures.

### Verify VMX is Available

Assuming you created a nested-enabled image using the previous instructions, verify that VMs created from this image are VMX-enabled with the following:

1. Create a VM from the image created previously:

    ```bash
    $ gcloud compute instances create \
        --image $IMAGE_NAME \
        --machine-type n1-standard-2 \
        --min-cpu-platform "Intel Broadwell" \
        kata-testing
    ```

> **NOTE**: In most zones the `--min-cpu-platform` argument can be omitted. It is only necessary in GCE Zones that include hosts based on Intel's Ivybridge platform.

2. Verify that the VMX CPUID flag is set:

    ```bash
    $ gcloud compute ssh kata-testing
    
    # While ssh'd into the VM:
    $ [ -z "$(lscpu|grep GenuineIntel)" ] && { echo "ERROR: Need an Intel CPU"; exit 1; }
    ```

If this fails, ensure you created your instance from the correct image and that the previously listed `enable-vmx` license is included.

## Install Kata

The process for installing Kata itself on a virtualization-enabled VM is identical to that for bare metal.

For detailed information to install Kata on your distribution of choice, see the [Kata Containers installation user guides](../install/README.md).

## Create a Kata-enabled Image

Optionally, after installing Kata, create an image to preserve the fruits of your labor:

```bash
$ gcloud compute instances stop kata-testing
$ gcloud compute images create \
    --source-disk kata-testing \
    kata-base
```

The result is an image that includes any changes made to the `kata-testing` instance as well as the `enable-vmx` flag. Verify this with `gcloud compute images describe kata-base`. The result, which omits some fields for clarity, should be similar to the following:

```yaml
diskSizeGb: '10'
kind: compute#image
licenseCodes:
  - '1002001'
  - '5926592092274602096'
licenses:
  - https://www.googleapis.com/compute/v1/projects/vm-options/global/licenses/enable-vmx
  - https://www.googleapis.com/compute/v1/projects/ubuntu-os-cloud/global/licenses/ubuntu-1804-lts
name: kata-base
selfLink: https://www.googleapis.com/compute/v1/projects/my-kata-project/global/images/kata-base
sourceDisk: https://www.googleapis.com/compute/v1/projects/my-kata-project/zones/us-west1-a/disks/kata-testing
sourceType: RAW
status: READY
```
