* [Run the osbuilder tests](#run-the-osbuilder-tests)
* [Further information](#further-information)

## Run the osbuilder tests

osbuilder provides a test script that creates all images and initrds for all
supported distributions and then tests them to ensure a Kata Container can
be created with each.

The test script installs all required Kata components on the host system
before creating the images.

To run all available osbuilder tests:

```
$ ./test_images.sh
```

## Further information

The test script provides various options to modify the way it runs. For full
details:

```
$ ./test_images.sh -h
```
