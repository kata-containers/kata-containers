* [Run the osbuilder tests](#run-the-osbuilder-tests)
* [Further information](#further-information)

## Run the osbuilder tests

osbuilder provides a test script that creates all rootfs disk images and
initrd images for all supported distributions and then tests them to ensure a
Kata Container can be created with each.

Before the build phase, the test script installs the Docker container manager
and all the Kata components required to run test containers. Individual tests
will also alter host `kata-runtime` and `docker` service configuration as needed.

All host config editing can be skipped by setting the environment variable
`KATA_DEV_MODE` to a non-empty value. In this mode, image/initrd targets
will be built but not runtime tested; If your host is configured to have
`kata-runtime` set as the default docker runtime, you will need to switch
to a runtime like `runc`/`crun` so the `docker build` test commands work
correctly.

```
$ ./test_images.sh
```

## Further information

The test script provides various options to modify the way it runs. For full
details:

```
$ ./test_images.sh -h
```
