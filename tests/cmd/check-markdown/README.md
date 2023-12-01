# Overview

The Kata Project comprises
[a number of GitHub repositories](https://github.com/kata-containers).
All these repositories contain documents written in
[GitHub-Flavoured Markdown](https://github.github.com/gfm)
format.

[Linking in documents is strongly encouraged](https://github.com/kata-containers/kata-containers/blob/main/docs/Documentation-Requirements.md)
but due to the number of internal and external document links, it is easy for
mistakes to be made. Also, links can become stale when one document is updated
but the documents it depends on are not.

# Tool summary

The `kata-check-markdown` tool checks a markdown document to ensure all links
within it are valid. All internal links are checked and by default all
external links are also checked. The tool is able to suggest corrections for
some errors it finds. It can also generate a TOC (table of contents).

# Usage

## Basic

```sh
$ kata-check-markdown check README.md
```

## Generate a TOC

```sh
$ kata-check-markdown toc README.md
```

## List headings

To list the document headings in the default `text` format:

```sh
$ kata-check-markdown list headings README.md
```

## List links

To list the links in a document in tab-separated format:

```sh
$ kata-check-markdown list links --format tsv README.md
```

## Full details

Lists all available options:

```sh
$ kata-check-markdown -h
```
