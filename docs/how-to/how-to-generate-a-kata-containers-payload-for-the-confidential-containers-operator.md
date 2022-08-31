# Generating a Kata Containers payload for the Confidential Containers Operator

[Confidential Containers
Operator](https://github.com/confidential-containers/operator) consumes a Kata
Containers payload, generated from the `CCv0` branch, and here one can find all
the necessary info on how to build such a payload.

## Requirements

* `make` installed in the machine
* Docker installed in the machine
* `sudo` access to the machine

## Process

* Clone [Kata Containers](https://github.com/kata-containers/kata-containers)
  ```sh
  git clone --branch CCv0 https://github.com/kata-containers/kata-containers
  ```
  * In case you've already cloned the repo, make sure to switch to the `CCv0` branch
    ```sh
    git checkout CCv0
    ```
  * Ensure your tree is clean and in sync with upstream `CCv0`
    ```sh
    git clean -xfd
    git reset --hard <upstream>/CCv0
    ```
* Make sure you're authenticated to `quay.io`
    ```sh
    sudo docker login quay.io
    ```
* From the top repo directory, run:
  ```sh
  sudo make cc-payload
  ```
* Make sure the image was upload to the [Confidential Containers
  runtime-payload
registry](https://quay.io/repository/confidential-containers/runtime-payload?tab=tags)

## Notes

Make sure to run it on a machine that's not the one you're hacking on, prepare a
cup of tea, and get back to it an hour later (at least).
