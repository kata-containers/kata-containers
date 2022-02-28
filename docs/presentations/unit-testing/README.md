# Kata Containers unit testing presentation

## Markdown version

See [the Kata Containers unit testing presentation](kata-containers-unit-testing.md).

### To view as an HTML presentation

```bash
$ infile="kata-containers-unit-testing.md"
$ outfile="/tmp/kata-containers-unit-testing.html"
$ pandoc -s --metadata title="Kata Containers unit testing" -f markdown -t revealjs --highlight-style="zenburn" -i -o "$outfile" "$infile"
$ xdg-open "file://$outfile"
```
