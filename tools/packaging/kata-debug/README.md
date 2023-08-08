# kata-debug

`kata-debug` is a tool that is used as part of the Kata Containers CI to gather
information from the node, in order to help debugging issues with Kata
Containers.

As one can imagine, this can be expanded and used outside of the CI context,
and any contribution back to the script is very much welcome.

The resulting container is stored at the [Kata Containers `quay.io`
space](https://quay.io/repository/kata-containers/kata-debug) and can
be used as shown below:
```sh
kubectl debug $NODE_NAME -it --image=quay.io/kata-containers/kata-debug:latest
```

## Building and publishing
The project can be built and publish by calling the following command from the
Kata Containers top directory:
```sh
make build-and-publish-kata-debug
```

Users can specify the following environment variables to the build:
* `KATA_DEBUG_REGISTRY` - The container registry to be used
                          default: `quay.io/kata-containers/kata-debug`
- `KATA_DEBUG_TAG`      - A tag to the be used for the image
                          default: `$(git rev-parse HEAD)-$(uname -a)`
