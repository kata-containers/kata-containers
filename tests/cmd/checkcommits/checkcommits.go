// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/urfave/cli"
)

// CommitConfig encapsulates the user configuration options, but is also
// used to pass some state between functions (FoundFixes).
type CommitConfig struct {
	// set when a "Fixes #XXX" commit is found
	FoundFixes bool

	// All commits must have a sign-off
	NeedSOBS bool

	// At least one commit must specify a bug that it fixes.
	NeedFixes bool

	MaxSubjectLineLength int
	MaxBodyLineLength    int

	SobString   string
	FixesString string

	// Ignore NeedFixes if the subsystem matches this value.
	IgnoreFixesSubsystem string

	FixesPattern *regexp.Regexp
	SobPattern   *regexp.Regexp
}

// Commit represents a git(1) commit
type Commit struct {
	hash      string
	subject   string
	subsystem string
	body      []string
}

const (
	defaultSobString   = "Signed-off-by"
	defaultFixesString = "Fixes"

	defaultMaxSubjectLineLength = 75
	defaultMaxBodyLineLength    = 72

	defaultCommit = "HEAD"
	defaultBranch = "master"

	versionSuffix = "for kata-containers"
)

var (
	// Full path to git(1) command
	gitPath = ""
	verbose = false
	debug   = false

	// XXX: set by build
	appCommit  = ""
	appVersion = ""

	errNoCommit = errors.New("Need commit")
	errNoBranch = errors.New("Need branch")
	errNoConfig = errors.New("Need config")
)

func init() {
	var err error
	gitPath, err = exec.LookPath("git")
	if err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: cannot find git in PATH\n")
		os.Exit(1)
	}
}

func commonChecks(config *CommitConfig, commit *Commit) error {
	if config == nil {
		return errNoConfig
	}

	if commit == nil {
		return errNoCommit
	}

	return nil
}

func checkCommitSubject(config *CommitConfig, commit *Commit) error {
	if err := commonChecks(config, commit); err != nil {
		return err
	}

	subject := commit.subject

	if subject == "" {
		return fmt.Errorf("Commit %v: empty subject", commit.hash)
	}

	if strings.TrimSpace(subject) == "" {
		return fmt.Errorf("Commit %v: pure whitespace subject", commit.hash)
	}

	subsystemPattern := regexp.MustCompile(`^[[:blank:]]*([^:[:blank:]]*)[[:blank:]]*:`)

	matches := subsystemPattern.FindStringSubmatch(subject)

	var subsystem string

	if matches == nil || len(matches) != 2 {
		return fmt.Errorf("Commit %v: Failed to find subsystem in subject: %q", commit.hash, subject)
	}

	// matches[0]: the entire matching string
	// matches[1] the subsystem name (without the colon)
	subsystem = matches[1]

	length := len(subject)
	if length > config.MaxSubjectLineLength {
		return fmt.Errorf("Commit %v: subject too long (max %v, got %v): %q",
			commit.hash, config.MaxSubjectLineLength, length, subject)
	}

	commit.subsystem = subsystem

	if config.NeedFixes && config.FixesPattern != nil {
		matches = config.FixesPattern.FindStringSubmatch(subject)

		if matches != nil {
			config.FoundFixes = true
		}
	}

	return nil
}

func checkCommitBodyLine(config *CommitConfig, commit *Commit, line string,
	lineNum int, nonWhitespaceOnlyLine *int,
	sobLine *int) error {

	if err := commonChecks(config, commit); err != nil {
		return err
	}

	if line == "" {
		return nil
	}

	// Remove all whitespace
	trimmedLine := strings.TrimSpace(line)

	if *nonWhitespaceOnlyLine == -1 {
		if trimmedLine != "" {
			*nonWhitespaceOnlyLine = lineNum
		}
	}

	if config.NeedFixes && config.FixesPattern != nil {
		fixesMatches := config.FixesPattern.FindStringSubmatch(line)
		if fixesMatches != nil {
			config.FoundFixes = true
		}
	}

	if config.NeedSOBS {
		sobMatch := config.SobPattern.FindStringSubmatch(line)
		if sobMatch != nil {
			*sobLine = lineNum
		}
	}

	// Note: SOB lines are *NOT* checked for max line
	// length: it isn't reasonable to penalise someone
	// people with long names ;)
	if *sobLine != -1 {
		return nil
	}

	// Check first character of line. If it's _not_
	// alphabetic, length limits don't apply.
	rune, _ := utf8.DecodeRune([]byte{line[0]})

	if !unicode.IsLetter(rune) {
		return nil
	}

	// If the line comprises of only a single word, it may be
	// something like a URL (it's certainly very unlikely to be a
	// normal word if the default lengths are being used), so length
	// checks won't be applied to it.
	singleWordLine := false
	if trimmedLine == line {
		singleWordLine = true
	}

	length := len(line)
	if length > config.MaxBodyLineLength && !singleWordLine {
		return fmt.Errorf("commit %v: body line %d too long (max %v, got %v): %q",
			commit.hash, 1+lineNum, config.MaxBodyLineLength, length, line)
	}

	return nil
}

func checkCommitBody(config *CommitConfig, commit *Commit) error {
	if err := commonChecks(config, commit); err != nil {
		return err
	}

	body := commit.body
	if body == nil {
		return fmt.Errorf("Commit %v: empty body", commit.hash)
	}

	// line number which contains a sign-off line.
	sobLine := -1

	// line number containing only whitespace
	nonWhitespaceOnlyLine := -1

	for i, line := range body {
		err := checkCommitBodyLine(config, commit, line, i,
			&nonWhitespaceOnlyLine, &sobLine)
		if err != nil {
			return err
		}
	}

	if nonWhitespaceOnlyLine == -1 {
		return fmt.Errorf("Commit %v: pure whitespace body", commit.hash)
	}

	if config.NeedSOBS && sobLine == -1 {
		return fmt.Errorf("Commit %v: no %v specified", commit.hash, config.SobString)
	}

	if sobLine == nonWhitespaceOnlyLine {
		return fmt.Errorf("Commit %v: single-line %q body not permitted", commit.hash, config.SobString)
	}

	return nil
}

func getCommitRange(commit, branch string) ([]string, error) {
	if commit == "" {
		return nil, errNoCommit
	}

	if branch == "" {
		return nil, errNoBranch
	}

	var args []string

	args = append(args, gitPath)
	args = append(args, "rev-list")
	args = append(args, "--no-merges")
	args = append(args, "--reverse")
	args = append(args, fmt.Sprintf("origin/%s..%s", branch, commit))

	return runCommand(args)
}

func runGitLog(commit, prettyFormat string) ([]string, error) {
	if commit == "" {
		return nil, errNoCommit
	}

	if prettyFormat == "" {
		return nil, errors.New("no pretty format")
	}

	var args []string

	args = append(args, gitPath)
	args = append(args, "log")
	args = append(args, "-1")
	args = append(args, fmt.Sprintf("--pretty=%s", prettyFormat))
	args = append(args, commit)

	return runCommand(args)
}

func getCommitSubject(commit string) (string, error) {
	if commit == "" {
		return "", errNoCommit
	}

	lines, err := runGitLog(commit, "%s")
	if err != nil {
		return "", err
	}

	return lines[0], nil
}

func getCommitBody(commit string) ([]string, error) {
	if commit == "" {
		return []string{}, errNoCommit
	}

	return runGitLog(commit, "%b")
}

func checkCommit(config *CommitConfig, commit *Commit) error {
	err := checkCommitSubject(config, commit)
	if err != nil {
		return err
	}

	return checkCommitBody(config, commit)
}

// checkCommits performs checks on specified list of commits
func checkCommits(config *CommitConfig, commitHashes []string) error {
	if config == nil {
		return errNoConfig
	}

	if commitHashes == nil {
		return errNoCommit
	}

	if len(commitHashes) == 0 {
		// Handle Travis builds on master
		return nil
	}

	commits, err := getCommits(commitHashes)
	if err != nil {
		return err
	}

	return checkCommitsDetails(config, commits)
}

func getCommits(commitHashes []string) ([]Commit, error) {
	if commitHashes == nil {
		return []Commit{}, errNoCommit
	}

	var commits []Commit

	for _, hash := range commitHashes {
		subject, err := getCommitSubject(hash)
		if err != nil {
			return []Commit{}, err
		}

		if subject == "" {
			return []Commit{}, fmt.Errorf("Commit %v: empty subject", hash)
		}

		body, err := getCommitBody(hash)
		if err != nil {
			return []Commit{}, err
		}

		if body == nil {
			return []Commit{}, fmt.Errorf("Commit %v: empty body", hash)
		}

		commit := Commit{
			hash:    hash,
			subject: subject,
			body:    body,
		}

		commits = append(commits, commit)
	}

	return commits, nil
}

func checkCommitsDetails(config *CommitConfig, commits []Commit) (err error) {
	if config == nil {
		return errNoConfig
	}

	if commits == nil {
		return errNoCommit
	}

	var results []Commit

	for _, commit := range commits {
		err = checkCommit(config, &commit)
		if err != nil {
			return err
		}

		results = append(results, commit)
	}

	ignoreFixesSubsystem := false

	if config.IgnoreFixesSubsystem != "" {
		for _, commit := range results {
			if strings.HasPrefix(commit.subsystem, config.IgnoreFixesSubsystem) {
				ignoreFixesSubsystem = true
			}
		}
	}

	if ignoreFixesSubsystem {
		// Fixes isn't required for the entire commit range
		// due to the specified subsystem being found in one of the
		// commits.
		config.NeedFixes = false
	}

	if config.NeedFixes && !config.FoundFixes {
		return fmt.Errorf("No %q found", config.FixesString)
	}

	return nil
}

// detectCIEnvironment checks if running under a recognised Continuous
// Integration system and returns the commit from the source ("from")
// branch, the destination ("to") branch (normally "master") that
// "commit" wants to be merged into, and the source ("from") branch.
//
// If srcBranch is unset the CI is handling a "master" branch (non-PR)
// build.
func detectCIEnvironment() (commit, dstBranch, srcBranch string) {
	var name string

	if os.Getenv("TRAVIS") != "" {
		name = "TravisCI"

		commit = os.Getenv("TRAVIS_PULL_REQUEST_SHA")

		srcBranch = os.Getenv("TRAVIS_PULL_REQUEST_BRANCH")
		dstBranch = os.Getenv("TRAVIS_BRANCH")

	} else if os.Getenv("SEMAPHORE") != "" {
		name = "SemaphoreCI"

		commit = os.Getenv("REVISION")

		dstBranch = os.Getenv("BRANCH_NAME")

		// Semaphore only has a single branch variable. For a PR
		// branch, it will contain the name of the PR branch,
		// but for a build of "master", that same variable will
		// contain "master". Essentially, the variable always
		// refers to the name of the current branch being built.
		if os.Getenv("PULL_REQUEST_NUMBER") != "" {
			srcBranch = dstBranch

			// Oddly, a git checkout for a PR under Semaphore *only*
			// contains that branch: master doesn't exist.
			dstBranch = "origin"
		}
	} else if os.Getenv("ghprbPullId") != "" {
		name = "JenkinsCI - github pull request builder"

		commit = os.Getenv("ghprbActualCommit")

		srcBranch = os.Getenv("ghprbSourceBranch")
		dstBranch = os.Getenv("ghprbTargetBranch")
	}

	if verbose && name != "" {
		fmt.Printf("Detected %v Environment\n", name)
	}

	return commit, dstBranch, srcBranch
}

// preChecks performs checks on the range of commits described by commit
// and branch.
func preChecks(config *CommitConfig, commit, branch string) error {
	if config == nil {
		return errNoConfig
	}

	if commit == "" {
		return errNoCommit
	}

	if branch == "" {
		return errNoBranch
	}

	commits, err := getCommitRange(commit, branch)
	if err != nil {
		return err
	}

	if verbose {
		l := len(commits)

		extra := ""
		if l != 1 {
			extra = "s"
		}

		fmt.Printf("Found %d commit%s between commit %v and branch %v\n",
			l, extra, commit, branch)
	}

	return checkCommits(config, commits)
}

// runCommand runs the command specified by args and returns its stdout
// lines as a slice.
func runCommand(args []string) (stdout []string, err error) {
	var outBytes, errBytes bytes.Buffer

	cmd := exec.Command(args[0], args[1:]...)

	cmdline := strings.Join(args, " ")
	if debug {
		fmt.Printf("Running: %q\n", cmdline)
	}

	cmd.Stdout = &outBytes
	cmd.Stderr = &errBytes

	err = cmd.Run()
	if err != nil {
		e := fmt.Errorf("Failed to run command %v: %v"+
			" (stdout: %v, stderr: %v)",
			cmdline, err, outBytes.String(), errBytes.String())
		return nil, e
	}

	lines := strings.Split(outBytes.String(), "\n")

	// Remove last line if empty
	length := len(lines)
	last := lines[length-1]
	if last == "" {
		lines = lines[:length-1]
	}

	return lines, nil
}

// NewCommitConfig creates a new CommitConfig object.
func NewCommitConfig(needFixes, needSignOffs bool, fixesPrefix, signoffPrefix, ignoreFixesForSubsystem string, bodyLength, subjectLength int) *CommitConfig {
	config := &CommitConfig{
		NeedSOBS:             needSignOffs,
		NeedFixes:            needFixes,
		MaxBodyLineLength:    bodyLength,
		MaxSubjectLineLength: subjectLength,
		SobString:            defaultSobString,
		FixesString:          defaultFixesString,
		IgnoreFixesSubsystem: ignoreFixesForSubsystem,
	}

	if config.MaxBodyLineLength == 0 {
		config.MaxBodyLineLength = defaultMaxBodyLineLength
	}

	if config.MaxSubjectLineLength == 0 {
		config.MaxSubjectLineLength = defaultMaxSubjectLineLength
	}

	if fixesPrefix != "" {
		config.FixesString = fixesPrefix
	}

	if signoffPrefix != "" {
		config.SobString = signoffPrefix
	}

	if config.NeedFixes {
		config.FixesPattern = regexp.MustCompile(fmt.Sprintf(`(?i:%s\s*:?\s*#\d+)`, config.FixesString))
	}

	if config.NeedSOBS {
		// note that sign-off lines must start in the first column
		config.SobPattern = regexp.MustCompile(fmt.Sprintf("^%s:", config.SobString))
	}

	return config
}

// branchMatchesREList returns the matching pattern if branch is
// specified by any of the regular expressions in the slice, else "".
func branchMatchesREList(branch string, branches []string) string {
	if branch == "" {
		return ""
	}

	for _, pattern := range branches {
		re := regexp.MustCompile(pattern)

		matches := re.FindAllStringSubmatch(branch, -1)
		if matches != nil {
			return pattern
		}
	}

	return ""
}

// expandCommitAndBranch expands the specified commit and branch value,
// resolving them to default values where appropriate
func expandCommitAndBranch(originalCommit, originalBranch string) (commit, branch string) {
	commit = originalCommit
	branch = originalBranch

	if commit == "" {
		commit = defaultCommit

		if verbose {
			fmt.Printf("Defaulting commit to %s\n", commit)
		}
	}

	if branch == "" {
		branch = defaultBranch

		if verbose {
			fmt.Printf("Defaulting branch to %s\n", branch)
		}
	}

	return commit, branch
}

// getCommitAndBranch determines the commit and branch to use.
func getCommitAndBranch(args, srcBranchesToIgnore []string) (commit, branch string, err error) {
	var srcBranch string

	if args == nil {
		return "", "", errors.New("No args")
	}

	if srcBranchesToIgnore == nil {
		return "", "", errors.New("No source branches")
	}

	count := len(args)

	if count == 0 {
		// no arguments so check the environment
		commit, branch, srcBranch = detectCIEnvironment()
	}

	if count > 2 {
		return "", "", errors.New("Too many arguments. Run with '--help' for usage")
	}

	if commit == "" && count >= 1 {
		commit = args[0]
	}

	if branch == "" && count == 2 {
		branch = args[1]
	}

	commit, branch = expandCommitAndBranch(commit, branch)

	if srcBranch != "" {
		match := ignoreSrcBranch(commit, srcBranch, srcBranchesToIgnore)

		if match != "" {
			if verbose {
				fmt.Printf("Exiting as ignored source branch %q matched pattern %q.\n", srcBranch, match)
			}

			os.Exit(0)
		}
	}

	return commit, branch, nil
}

func getCommitAndBranchWithContext(c *cli.Context) (commit, branch string, err error) {
	return getCommitAndBranch(c.Args(), c.StringSlice("ignore-source-branch"))
}

func checkCommitsAction(c *cli.Context) error {
	if c.Bool("debug") {
		verbose = true
	}

	if verbose {
		fmt.Printf("Running %v version %s\n", c.App.Name, c.App.Version)
	}

	commit, branch, err := getCommitAndBranchWithContext(c)
	if err != nil {
		return err
	}

	config := NewCommitConfig(c.Bool("need-fixes"),
		c.Bool("need-sign-offs"),
		c.String("fixes-prefix"),
		c.String("sign-off-prefix"),
		c.String("ignore-fixes-for-subsystem"),
		int(c.Uint("body-length")),
		int(c.Uint("subject-length")))

	return preChecks(config, commit, branch)
}

func main() {
	app := cli.NewApp()
	app.Name = "checkcommits"
	app.Version = appVersion + " (commit " + appCommit + ")"
	app.Description = "perform checks on git commits"
	app.Usage = app.Description
	app.UsageText = fmt.Sprintf("%s [global options] [commit [branch]]\n", app.Name)
	app.UsageText += fmt.Sprintf("\n")
	app.UsageText += fmt.Sprintf("Notes:\n")
	app.UsageText += fmt.Sprintf("   - The commit argument refers to the (normally latest) commit in the\n")
	app.UsageText += fmt.Sprintf("     source branch that wants to be merged into the specified (destination)\n")
	app.UsageText += fmt.Sprintf("     branch.\n\n")
	app.UsageText += fmt.Sprintf("   - If not specified, commit and branch will be set automatically\n")
	app.UsageText += fmt.Sprintf("     if running in a supported CI environment (Travis or Semaphore).\n\n")
	app.UsageText += fmt.Sprintf("   - If not running under a recognised CI environment, commit will default\n")
	app.UsageText += fmt.Sprintf("     to %q and branch to %q.", defaultCommit, defaultBranch)

	cli.VersionPrinter = func(c *cli.Context) {
		// #nosec
		fmt.Fprintf(os.Stdout, "%s version %s %s\n",
			c.App.Name,
			c.App.Version,
			versionSuffix)
	}

	app.Flags = []cli.Flag{
		cli.BoolFlag{
			Name:  "need-fixes, f",
			Usage: fmt.Sprintf("Ensure at least one commit has a %q entry", defaultFixesString),
		},

		cli.BoolFlag{
			Name:  "need-sign-offs, s",
			Usage: fmt.Sprintf("Ensure all commits have a %q entry", defaultSobString),
		},

		cli.BoolFlag{
			Name:        "verbose",
			Usage:       "Display informational messages",
			EnvVar:      "CHECKCOMMITS_VERBOSE",
			Destination: &verbose,
		},

		cli.BoolFlag{
			Name:        "debug",
			Usage:       "Display debug messages (implies verbose)",
			EnvVar:      "CHECKCOMMITS_DEBUG",
			Destination: &debug,
		},

		cli.StringFlag{
			Name:  "ignore-fixes-for-subsystem",
			Usage: fmt.Sprintf("Don't requires a Fixes comment if the subsystem matches the specified string"),
		},

		cli.StringFlag{
			Name:  "fixes-prefix",
			Usage: fmt.Sprintf("Fixes `prefix` used as an alternative to %q", defaultFixesString),
		},

		cli.StringFlag{
			Name:  "sign-off-prefix",
			Usage: fmt.Sprintf("Sign-off `prefix` used as an alternative to %q", defaultSobString),
		},

		cli.StringSliceFlag{
			Name:  "ignore-source-branch",
			Usage: "regular expression `regex` representing source branch to ignore (can be specified multiple times)",
		},

		cli.UintFlag{
			Name:  "body-length",
			Usage: "Specify maximum body line `length`",
			Value: uint(defaultMaxBodyLineLength),
		},

		cli.UintFlag{
			Name:  "subject-length",
			Usage: "Specify maximum subject line `length`",
			Value: uint(defaultMaxSubjectLineLength),
		},
	}

	app.Action = checkCommitsAction

	err := app.Run(os.Args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: %v\n", err)
		os.Exit(1)
	}

	if verbose {
		fmt.Printf("All commit checks passed.\n")
	}

	os.Exit(0)
}

// ignoreSrcBranch returns the matching regular expression pattern from
// branchesToIgnore for a match or "" if no match.
func ignoreSrcBranch(commit, srcBranch string, branchesToIgnore []string) string {
	if commit == "" {
		return ""
	}

	if branchesToIgnore == nil {
		return ""
	}

	if srcBranch == "" {
		// This is a non-PR build so it is not possible to
		// ignore any branches.
		if verbose {
			fmt.Printf("WARNING: cannot use ignore branches on %v\n", defaultBranch)
		}

		return ""
	}

	return branchMatchesREList(srcBranch, branchesToIgnore)
}
