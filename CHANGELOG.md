# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.15.2](https://github.com/francisdb/vpin/compare/v0.15.1...v0.15.2) - 2024-06-14

### Added
- images to webp ([#92](https://github.com/francisdb/vpin/pull/92))

### Other
- *(deps)* update quick-xml requirement from 0.31.0 to 0.32.0 ([#90](https://github.com/francisdb/vpin/pull/90))

## [0.15.1](https://github.com/francisdb/vpin/compare/v0.15.0...v0.15.1) - 2024-06-09

### Fixed
- timer interval signed ([#88](https://github.com/francisdb/vpin/pull/88))

## [0.15.0](https://github.com/francisdb/vpin/compare/v0.14.5...v0.15.0) - 2024-06-03

### Added
- more/open tonemapper values ([#85](https://github.com/francisdb/vpin/pull/85))

## [0.14.5](https://github.com/francisdb/vpin/compare/v0.14.4...v0.14.5) - 2024-05-28

### Added
- sound output type enum ([#84](https://github.com/francisdb/vpin/pull/84))

### Other
- *(deps)* update roxmltree requirement from 0.19.0 to 0.20.0 ([#82](https://github.com/francisdb/vpin/pull/82))

## [0.14.4](https://github.com/francisdb/vpin/compare/v0.14.3...v0.14.4) - 2024-05-10

### Added
- *(exported)* more redundant img fields removed ([#79](https://github.com/francisdb/vpin/pull/79))

## [0.14.3](https://github.com/francisdb/vpin/compare/v0.14.2...v0.14.3) - 2024-05-09

### Fixed
- *(extracted)* unknown image extension ([#77](https://github.com/francisdb/vpin/pull/77))

## [0.14.2](https://github.com/francisdb/vpin/compare/v0.14.1...v0.14.2) - 2024-05-09

### Added
- *(extracted)* do not write dimensions in json if not needed ([#75](https://github.com/francisdb/vpin/pull/75))

## [0.14.1](https://github.com/francisdb/vpin/compare/v0.14.0...v0.14.1) - 2024-05-09

### Fixed
- *(vpx)* keep alpha channels for rgba bmp ([#73](https://github.com/francisdb/vpin/pull/73))

## [0.14.0](https://github.com/francisdb/vpin/compare/v0.13.0...v0.14.0) - 2024-05-08

### Fixed
- *(vpx)* bmp image export/import ([#69](https://github.com/francisdb/vpin/pull/69))

## [0.13.0](https://github.com/francisdb/vpin/compare/v0.12.0...v0.13.0) - 2024-04-24

### Added
- improve json color fields, add gamedata enums ([#66](https://github.com/francisdb/vpin/pull/66))

### Other
- back to macos-latest ([#67](https://github.com/francisdb/vpin/pull/67))
- pin to macOS-13
- example for programmatic table creation ([#65](https://github.com/francisdb/vpin/pull/65))
- document discord channel

## [0.12.0](https://github.com/francisdb/vpin/compare/v0.11.2...v0.12.0) - 2024-04-19

### Added
- improve enum values json representation ([#61](https://github.com/francisdb/vpin/pull/61))

## [0.11.2](https://github.com/francisdb/vpin/compare/v0.11.1...v0.11.2) - 2024-04-15

### Added
- strict cfb file reading

### Other
- *(deps)* update cfb requirement from 0.9.0 to 0.10.0 ([#60](https://github.com/francisdb/vpin/pull/60))
- also assert eq cfb version/clsid
- add cache for clippy build
- move fmt check to clippy action
- add clippy action ([#58](https://github.com/francisdb/vpin/pull/58))

## [0.11.1](https://github.com/francisdb/vpin/compare/v0.11.0...v0.11.1) - 2024-04-02

### Other
- clippy cleanup

## [0.11.0](https://github.com/francisdb/vpin/compare/v0.10.2...v0.11.0) - 2024-04-02

### Fixed
- handling symbol fonts ([#54](https://github.com/francisdb/vpin/pull/54))

## [0.10.2](https://github.com/francisdb/vpin/compare/v0.10.1...v0.10.2) - 2024-04-02

### Added
- unify drag_points fields

### Fixed
- *(vpx)* tags that require 0 size ([#53](https://github.com/francisdb/vpin/pull/53))
- *(expanded)* correctly update mesh info ([#51](https://github.com/francisdb/vpin/pull/51))
- textbox/decal FONT tag location ([#50](https://github.com/francisdb/vpin/pull/50))

## [0.10.1](https://github.com/francisdb/vpin/compare/v0.10.0...v0.10.1) - 2024-03-26

### Fixed
- JPEG tag should have size 0 ([#47](https://github.com/francisdb/vpin/pull/47))

## [0.10.0](https://github.com/francisdb/vpin/compare/v0.9.0...v0.10.0) - 2024-03-21

### Fixed
- serialization issues ([#45](https://github.com/francisdb/vpin/pull/45))

## [0.9.0](https://github.com/francisdb/vpin/compare/v0.8.0...v0.9.0) - 2024-03-19

### Added
- extracted vpx structure ([#21](https://github.com/francisdb/vpin/pull/21))

### Other
- *(deps)* update testresult requirement from 0.3.0 to 0.4.0 ([#36](https://github.com/francisdb/vpin/pull/36))
- *(deps)* bump actions/cache from 3 to 4 ([#34](https://github.com/francisdb/vpin/pull/34))

## [0.8.0](https://github.com/francisdb/vpin/compare/v0.7.0...v0.8.0) - 2024-01-12

### Added
- support for brst field ([#32](https://github.com/francisdb/vpin/pull/32))

## [0.7.0](https://github.com/francisdb/vpin/compare/v0.6.0...v0.7.0) - 2023-11-29

### Added
- more vpinball 10.8 changes ([#30](https://github.com/francisdb/vpin/pull/30))

### Other
- *(deps)* update testdir requirement from 0.8.1 to 0.9.0 ([#29](https://github.com/francisdb/vpin/pull/29))
- *(deps)* update roxmltree requirement from 0.18.1 to 0.19.0 ([#26](https://github.com/francisdb/vpin/pull/26))

## [0.6.0](https://github.com/francisdb/vpin/compare/v0.5.0...v0.6.0) - 2023-11-09

### Other
- drop pov module as pov support removed in vpinball 10.8 ([#24](https://github.com/francisdb/vpin/pull/24))
- *(deps)* update cfb requirement from 0.8.1 to 0.9.0 ([#23](https://github.com/francisdb/vpin/pull/23))

## [0.5.0](https://github.com/francisdb/vpin/compare/v0.4.0...v0.5.0) - 2023-10-24

### Other
- directb2s use ([#19](https://github.com/francisdb/vpin/pull/19))

## [0.4.0](https://github.com/francisdb/vpin/compare/v0.3.0...v0.4.0) - 2023-10-23

### Added
- feat/directb2s improvements2 ([#15](https://github.com/francisdb/vpin/pull/15))

### Other
- *(deps)* update quick-xml requirement from 0.30.0 to 0.31.0 ([#16](https://github.com/francisdb/vpin/pull/16))

## [0.3.0](https://github.com/francisdb/vpin/compare/v0.2.0...v0.3.0) - 2023-10-20

### Added
- directb2s improvements ([#12](https://github.com/francisdb/vpin/pull/12))

### Other
- update release section in readme
- set up automatic releases
- update release section in README.md
