# Changelog

## v0.13.0

### Changed

- Use follows from instead of child of for links #524
- Remove default surf features #546
- Update to opentelemetry v0.14.0

## v0.12.1

### Fixed

- jaeger span error reporting and spec compliance #489

## v0.12.0

### Added
- Add max packet size constraint #457

### Fixed
- Allow user to use hostname like `localhost` in the `OTEL_EXPORTER_JAEGER_AGENT_HOST` environment variable. #448

### Removed 
- Removed `from_env` and use environment variables to initialize the configurations by default #459

### Changed
- Update to opentelemetry v0.13.0
- Rename trace config with_default_sampler to with_sampler #482

## v0.11.0

### Changed

- Update to opentelemetry v0.12.0
- Update tokio to v1 #421
- Make `with_collector_endpoint` function less error prune #428
- Use opentelemetry-http for http integration #415

## v0.10.0

### Added

- Add wasm support #365
- Allow user to use their own http clients or use 4 of the default implementation
  (`surf_collector_client`, `reqwest_collector_client`, `reqwest_blocking_collector_client`, `isahc_collector_client`)
- Set `otel.status_code` and `otel.status_description` values #383

### Changed

- Update to opentelemetry v0.11.0
- Use http client trait #378

## v0.9.0

### Added

- Option to disable exporting instrumentation library information #288

### Changed

- Update to opentelemetry v0.10.0
- Update mapping otel events to Jaeger logs attributes #285
- Add MSRV 1.42.0 #296

## v0.8.0

### Added

- Map `Resource`s to jaeger process tags #215
- Export instrument library information #243

### Changed

- Switch to pipeline configuration #189
- Update to opentelemetry v0.9.0

## v0.7.0

### Changed

- Update to opentelemetry v0.8.0

## v0.6.0

### Changed
- Update to opentelemetry v0.7.0

### Fixed
- Do not add `span.kind` tag if it has been set as an attribute #140

## v0.5.0

### Changed
- Update to opentelemetry v0.6.0

### Fixed
- Switch internally to `ureq` from `reqwest` to fix #106
- Fix exported link span id format #118

## v0.4.0

### Added
- Support for resource attributes

### Changed
- Update to opentelemetry v0.5.0

### Removed
- `as_any` method on exporter

## v0.3.0

### Changed
- Update to opentelemetry v0.4.0

## v0.2.0

### Changed
- Update to opentelemetry v0.3.0

## v0.1.0

### Added
- Jaeger agent Thrift UDP client
- Jaeger collector Thrift HTTP client
