# Scenarios – Executing a Command in Different Environments

[![Build Status](https://travis-ci.org/troiganto/scenarios.svg?branch=master)](https://travis-ci.org/troiganto/scenarios)

`Scenarios` is a command-line tool that allows you to execute the same command
multiple times, each time with different environment variables set. When passed
multiple lists of environments, `scenarios` goes through all possible
combinations between them.

`scenarios` is available on [Github].

[Github]: https://github.com/troiganto/scenarios

# Motivation

This tool was written to get a handle on a rather large, single-threaded
simulation software here-to-be-unnamed. The software could be configured
through a plethora of environment variables (all with sensible default values).

With `scenarios`, it was possible to run the simulation several times in
parallel, each instance configured in a different manner. This, in turn, meant
one could get and compare the simulation of many different *scenarios* for the
cost of one.

# Installation

`scenarios` is written in [Rust]. If you have already installed Rust, you can
simply clone this repository and call `cargo build --release`. Once this
program has stabilized a bit more, it will be distributed via [crates.io] and
pre-compiled binaries.

[Rust]: https://rust-lang.org/
[crates.io]: https://crates.io/

# Basic Usage

The general pattern of `scenarios`'s command line is:

```shell
scenarios [OPTIONS] <scenario files...> -- <command line>
```

A *scenario file* is a text file that defines different *scenarios* in an
INI-file-like format. A *scenario* is simply a set of environment variable
declarations with a name attached to them.

Assume you have a scenario file named `translations.ini` that looks like this:

```ini
[English]
test = test
example = example

[German]
test = Test
example = Beispiel
```

If you pass only the scenario file to `scenarios`, it will simply print the
scenarios' names back to stdout:

```shell
$ scenarios translations.ini
English
German
```

If you pass a command line to `scenarios` (separated from the scenarios file
with `--`), scenarios sets the environment variables of the first scenario,
executes the command, then sets the environment variables of the second
scenario, and executes the command again:

```shell
$ scenarios translations.ini -- sh -c 'echo "word = $example"'
word = example
word = Beispiel
```

Note that each invokation of the given command is a completely independent
process with its own environment; variables don't "carry over" from one
scenario to the next.

## A Note About Visibility

You might wonder why the command line in the previous example is `sh -c "echo
$variable"`, and not simply `echo $variable`. The problem is: if we simply
wrote `scenarios translations.ini -- echo $example`, then the shell would
expand `$example` before `scenarios` is even started. On the other hand, if we
wrote `scenarios translations.ini -- echo \$example`, the argument would
*never* be considered a shell variable and `echo` would simply print literally
`$example` twice.

Wrapping the variable expansion in a subshell moves the moment of the variable
being expanded from the current shell into the shell that is started by
`scenarios`, which is the only shell where the variable `example` is actually
set.

Usually, you wouldn't use `scenarios` like this. It is much cleaner to write
your own script in a language of your choosing (Bash, Python, or even Rust) and
have your script read the environment variables that `scenarios` has set:

```shell
scenarios translations.ini -- my_executable
```

# Advanced Usage

What makes `scenarios` actually powerful is that you can pass more than one
scenario file to it.

Assume we have a file `numbers.ini` like this:

```ini
[Number One]
number = 1
[Number Two]
number = 2
[Number Three]
number = 3
[Number Four]
number = 4
```

and a file `letters.ini` like this:

```ini
[Letter A]
letter = a
[Letter B]
letter = b
[Letter C]
letter = c
[Letter D]
letter = d
```

If we pass both files to `scenarios`, it will take one scenario from each file
and merge the two scenarios into one. *And* it will do that for each possible
combination of scenarios!

```shell
$ scenarios numbers.ini letters.ini -- sh -c 'echo "$number-$letter"'
1-a
1-b
1-c
1-d
2-a
2-b
2-c
2-d
3-a
3-b
3-c
3-d
4-a
4-b
4-c
4-d
```

The names of the scenarios get merged as well:

```shell
$ scenarios numbers.ini letters.ini
Number One, Letter A
Number One, Letter B
Number One, Letter C
Number One, Letter D
Number Two, Letter A
Number Two, Letter B
Number Two, Letter C
Number Two, Letter D
Number Three, Letter A
Number Three, Letter B
Number Three, Letter C
Number Three, Letter D
Number Four, Letter A
Number Four, Letter B
Number Four, Letter C
Number Four, Letter D
```

# Bells and Whistles

A small selection of additional things which make `scenarios` more usable:

- With the option `--jobs=N`, up to `N` scenarios are executed in parallel. If
  you just pass `--jobs`, `scenarios` runs as many processes in parallel as it
  thinks your computer has CPUs.

- If the given command fails for any scenario, `scenarios` usually exits
  immediately. You can, however, pass `--keep-going` to tell `scenarios` to go
  through all scenarios regardless of any errors.

- By default, `scenarios` replaces empty braces `{}` in your command line with
  the name of the current scenario. That means `scenarios <files> -- echo {}`
  is the same as `scenarios <files>`. (This can be turned off with
  `--no-insert-name`.)

- By default, `scenarios` *also* exports the current scenario's name as an
  additional environment variable, `SCENARIOS_NAME`. (This can be turned off
  with `--no-export-name`.)

- If you just want the scenario names printed, you can customize printing with
  the `--print` option. It takes a template string in which `{}` is replaced
  with the scenario names.

- To use `scenarios` in conjunction with `xargs`, you can use the option
  `--print0`. With it, the scenario names are separated by `NUL` instead of
  end-of-line characters.

- The `--choose` parameter allows you to only a single scenario combination out
  of a long list that `scenarios` would usually produce. This is useful if
  there is a bug in your setup and you just want to do a quick run to find it.

- Similarly, the `--exclude` parameter allows you to skip a single scenario
  that you are not interested in.
