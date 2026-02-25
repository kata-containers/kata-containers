module github.com/kata-containers/tests

// Keep in sync with version in versions.yaml
go 1.25.7

// WARNING: Do NOT use `replace` directives as those break dependabot:
// https://github.com/kata-containers/kata-containers/issues/11020

require (
	github.com/olekukonko/tablewriter v0.0.6-0.20210304033056-74c60be0ef68
	github.com/russross/blackfriday/v2 v2.1.0
	github.com/sirupsen/logrus v1.9.3
	github.com/stretchr/testify v1.7.1
	github.com/urfave/cli v1.22.0
	gopkg.in/yaml.v2 v2.4.0
)

require (
	github.com/cpuguy83/go-md2man v1.0.10 // indirect
	github.com/davecgh/go-spew v1.1.1 // indirect
	github.com/mattn/go-runewidth v0.0.13 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	github.com/rivo/uniseg v0.2.0 // indirect
	github.com/russross/blackfriday v1.6.0 // indirect
	golang.org/x/sys v0.19.0 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
)

// WARNING: Do NOT use `replace` directives as those break dependabot:
// https://github.com/kata-containers/kata-containers/issues/11020
