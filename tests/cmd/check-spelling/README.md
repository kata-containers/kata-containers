# Spell check tool

## Overview

The `kata-spell-check.sh` tool is used to check a markdown file for
typographical (spelling) mistakes.

## Approach

The spell check tool is based on
[`hunspell`](https://github.com/hunspell/hunspell). It uses standard Hunspell
English dictionaries and supplements these with a custom Hunspell dictionary.
The document is cleaned of several entities before the spell-check begins.
These entities include the following:

- URLs
- Email addresses
- Code blocks
- Most punctuation
- GitHub userids

## Custom words

A custom dictionary is required to accept specific words that are either well
understood by the community or are defined in various document files, but do
not appear in standard dictionaries. The custom dictionaries allow those words
to be accepted as correct. The following lists common examples of such words:

- Abbreviations
- Acronyms
- Company names
- Product names
- Project names
- Technical terms

## Spell check a document file

```sh
$ ./kata-spell-check.sh check /path/to/file
```

> **Note:** If you have made local edits to the dictionaries, you may 
> [re-create the master dictionary files](#create-the-master-dictionary-files)
> as documented in the [Adding a new word](#adding-a-new-word) section, 
> in order for your local edits take effect.

## Other options

Lists all available options and commands:

```sh
$ ./kata-spell-check.sh -h
```

## Technical details

### Hunspell dictionary format

A Hunspell dictionary comprises two text files:

- A word list file

  This file defines a list of words (one per line). The list includes optional
  references to one or more rules defined in the rules file as well as optional
  comments. Specify fixed words (e.g. company names) verbatim. Enter “normal”
  words in their root form.

  The root form of a "normal" word is the simplest and shortest form of that
  word. For example, the following list of words are all formed from the root
  word "computer":

  - Computers
  - Computer’s
  - Computing
  - Computed

  Each word in the previous list is an example of using the word "computer" to
  construct said word through a combination of applying the following
  manipulations:

  - Remove one or more characters from the end of the word.
  - Add a new ending.

  Therefore, you list the root word "computer" in the word list file.

- A rules file

  This file defines named manipulations to apply to root words to form new
  words. For example, rules that make a root word plural.

### Source files

The rules file and the the word list file for the custom dictionary generate
from "source" fragment files in the [`data`](data/) directory.

All the fragment files allow comments using the hash (`#`) comment
symbol and all files contain a comment header explaining their content.

#### Word list file fragments

The `*.txt` files are word list file fragments. Splitting the word list
into fragments makes updates easier and clearer as each fragment is a
grouping of related terms. The name of the file gives a clue as to the
contents but the comments at the top of each file provide further
detail.

Every line that does not start with a comment symbol contains a single
word. An optional comment for a word may appear after the word and is
separated from the word by whitespace followed by the comment symbol:

```
word		# This is a comment explaining this particular word list entry.
```

You *may* suffix each word by a forward slash followed by one or more
upper-case letters. Each letter refers to a rule name in the rules file:

```
word/AC		# This word references the 'A' and 'C' rules.
```

#### Rules file

The [rules file](data/rules.aff) contains a set of general rules that can be
applied to one or more root words in the word list files. You can make
comments in the rules file.

For an explanation of the format of this file see
[`man 5 hunspell`](http://www.manpagez.com/man/5/hunspell)
([source](https://github.com/hunspell/hunspell/blob/master/man/hunspell.5)).

## Adding a new word

### Update the word list fragment

If you want to allow a new word to the dictionary,

- Check to ensure you do need to add the word

  Is the word valid and correct? If the word is a project, product,
  or company name, is the capitalization correct?

- Add the new word to the appropriate [word list fragment file](data).

  Specifically, if it is a general word, add the *root* of the word to
  the appropriate fragment file.

- Add a `/` suffix along with the letters for each rule to apply in order to
  add rules references.

### Optionally update the rules file

It should not generally be necessary to update the rules file since it
already contains rules for most scenarios. However, if you need to
update the file, [read the documentation carefully](#rules-file).

### Create the master dictionary files

Every time you change the dictionary files you must recreate the master
dictionary files:

```sh
$ ./kata-spell-check.sh make-dict
```

As a convenience, [checking a file](#spell-check-a-document-file) will
automatically create the database.

### Test the changes

You must test any changes to the [word list file
fragments](#word-list-file-fragments) or the [rules file](#rules-file)
by doing the following:

1. Recreate the [master dictionary files](#create-the-master-dictionary-files).

1. [Run the spell checker](#spell-check-a-document-file) on a file containing the
   words you have added to the dictionary.
