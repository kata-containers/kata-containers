# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## 2.7.0 - 2021-07-29

* Fix license field to be a valid SPDX expression
* Support u128/i128

## 2.6.0 - 2021-01-12

* Update crossbeam-channel to 0.5
* Expose the serialization capabilities

## 2.5.0 - 2020-01-29

* Fix compilation warnings
* Upgrade `crossbeam-channel`

## 2.4.0 - 2020-01-29

* Do not join() when dropping AsyncCore/AsyncGuard from worker thread
* Upgrade to thread_local v1
* Replace the std mpsc channel with a crossbeam channel
* Replace the std mpsc channels with a crossbeam channel
* add missing LICENSE files

## 2.3.0 - 2018-04-04

* Configurable overflow strategy (can now block or drop the messages silently).
* Configurable name of the background thread.

## 2.2.0 - 2017-07-23

* Experimental support for `nested-values` and `dynamic-keys`. **Note**:
  consider unstable.

## 2.1.0 - 2017-07-23

* Relicense under MPL/Apache/MIT
* Added `AsyncGuard`
* `build` to be deprecated in the future


## 2.0.1 - 2017-04-11
### Fixed

* Don't reverse the order of key-value pairs

## 2.0.0 - 2017-04-11
### Changed
* Update to slog to stable release
* Minor improvements around overflow documentation

## 2.0.0-3.0 - 2017-03-25

* Bump slog version to 2.0.0-3.0

## 2.0.0-2.0 - 2017-03-11

* Bump slog version to 2.0.0-2.0

## 0.2.0-alpha2 - 2017-02-23

* Update to latest `slog` version
* Misc changes

## 0.2.0-alpha1 - 2017-02-19
### Changed

* Fork from `slog-extra` to become a part of `slog v2`
