# Documentation Contributions

The documentation system is built using a static site generator called [mkdocs-materialx](https://jaywhj.github.io/mkdocs-materialx/), a fork of [mkdocs-material](https://squidfunk.github.io/mkdocs-material/).

## Local Serving

All documentation files are in the `docs/` subfolder. When modifying docs, you can run `make docs-serve` at the top-level of the project to build and serve them locally:

```
$ make docs-serve
INFO    -  [17:37:42] Serving on http://0.0.0.0:8000/kata-containers/
```

## Adding New Files

Markdown files should be organized in a flat topology as much as possible. The location of the markdown files will map directly its URL, so they should not be moved once they are created. The navigation structure of the static site is controlled via the `docs/.nav.yml` file.

## Configuration

The configuration of the build system is controlled through the `mkdocs.yaml` file. The reference for various parameters can be found at the [mkdocs-materialx](https://jaywhj.github.io/mkdocs-materialx/setup/index.html) website.

