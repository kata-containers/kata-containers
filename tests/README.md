* [Run the osbuilder tests](#run-the-osbuilder-tests)
* [Further information](#further-information)

## Run the osbuilder tests

osbuilder provides a test script that creates all images and initrds for all
supported distributions and then tests them to ensure a Kata Container can
be created with each.

Before the build phase, the test script installs the Docker container manager
and all the Kata components required to run test containers. This step can be
skipped by setting the environment variable `KATA_DEV_MODE` to a non-empty
value.

```
$ ./test_images.sh
```

## Further information

The test script provides various options to modify the way it runs. For full
details:

```
$ ./test_images.sh -h
```
