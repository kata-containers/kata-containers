# liboci-cli

This is a crate to parse command line arguments for OCI container
runtimes as specified in the [OCI Runtime Command Line
Interface](https://github.com/opencontainers/runtime-tools/blob/master/docs/command-line-interface.md).

## Implemented subcommands

| Command    | liboci-cli | CLI Specification | runc | crun | youki |
| :--------: | :--------: | :---------------: | :--: | :--: | :---: |
| create     | ✅         | ✅                | ✅   | ✅   | ✅    |
| start      | ✅         | ✅                | ✅   | ✅   | ✅    |
| state      | ✅         | ✅                | ✅   | ✅   | ✅    |
| kill       | ✅         | ✅                | ✅   | ✅   | ✅    |
| delete     | ✅         | ✅                | ✅   | ✅   | ✅    |
| checkpoint |            |                   | ✅   | ✅   |       |
| events     | ✅         |                   | ✅   |      | ✅    |
| exec       | ✅         |                   | ✅   | ✅   | ✅    |
| list       | ✅         |                   | ✅   | ✅   | ✅    |
| pause      | ✅         |                   | ✅   | ✅   | ✅    |
| ps         | ✅         |                   | ✅   | ✅   | ✅    |
| restore    |            |                   | ✅   | ✅   |       |
| resume     | ✅         |                   | ✅   | ✅   | ✅    |
| run        | ✅         |                   | ✅   | ✅   | ✅    |
| spec       | ✅         |                   | ✅   | ✅   | ✅    |
| update     |            |                   | ✅   | ✅   |       |
