# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.18.7](https://github.com/francisdb/vpin/compare/v0.18.6...v0.18.7) - 2025-12-11

### Other

- update read/write of reel_count, motor_steps, digit_range as floats ([#173](https://github.com/francisdb/vpin/pull/173))
- remove duplicate import
- *(deps)* bump actions/checkout from 5 to 6 ([#171](https://github.com/francisdb/vpin/pull/171))

## [0.18.6](https://github.com/francisdb/vpin/compare/v0.18.5...v0.18.6) - 2025-11-07

### Added

- logging during assembly ([#167](https://github.com/francisdb/vpin/pull/167))

### Fixed

- sound file name sanitation was only applied for write ([#169](https://github.com/francisdb/vpin/pull/169))
- image file name sanitation was only applied for write ([#168](https://github.com/francisdb/vpin/pull/168))
- sounds without extension handling ([#165](https://github.com/francisdb/vpin/pull/165))

## [0.18.5](https://github.com/francisdb/vpin/compare/v0.18.4...v0.18.5) - 2025-11-05

### Fixed

- correct flipper exposed fields ([#162](https://github.com/francisdb/vpin/pull/162))

## [0.18.4](https://github.com/francisdb/vpin/compare/v0.18.3...v0.18.4) - 2025-11-05

### Added

- make fields public ([#160](https://github.com/francisdb/vpin/pull/160))

## [0.18.3](https://github.com/francisdb/vpin/compare/v0.18.2...v0.18.3) - 2025-10-23

### Added

- reading a vpx from bytes ([#158](https://github.com/francisdb/vpin/pull/158))

## [0.18.2](https://github.com/francisdb/vpin/compare/v0.18.1...v0.18.2) - 2025-10-21

### Other

- *(deps)* update cfb requirement from 0.11.0 to 0.12.0 ([#157](https://github.com/francisdb/vpin/pull/157))
- *(deps)* update roxmltree requirement from 0.20.0 to 0.21.0 ([#154](https://github.com/francisdb/vpin/pull/154))
- *(deps)* update sanitize-filename requirement from 0.5 to 0.6 ([#155](https://github.com/francisdb/vpin/pull/155))

## [0.18.1](https://github.com/francisdb/vpin/compare/v0.18.0...v0.18.1) - 2025-10-01

### Added

- more logging ([#153](https://github.com/francisdb/vpin/pull/153))

### Other

- add clippy workflow permissions ([#152](https://github.com/francisdb/vpin/pull/152))
- leftover changes ([#150](https://github.com/francisdb/vpin/pull/150))
- set up rust workflow permissions ([#151](https://github.com/francisdb/vpin/pull/151))
- Update Clippy workflow to include pull requests
- reformat
- adds filename sanitation and logging ([#148](https://github.com/francisdb/vpin/pull/148))
- *(deps)* bump actions/checkout from 4 to 5 ([#146](https://github.com/francisdb/vpin/pull/146))
- new clippy rules ([#147](https://github.com/francisdb/vpin/pull/147))
- *(deps)* update cfb requirement from 0.10.0 to 0.11.0 ([#145](https://github.com/francisdb/vpin/pull/145))
- *(deps)* update quick-xml requirement from 0.37.0 to 0.38.0 ([#142](https://github.com/francisdb/vpin/pull/142))
- new clippy rules ([#143](https://github.com/francisdb/vpin/pull/143))

## [0.18.0](https://github.com/francisdb/vpin/compare/v0.17.6...v0.18.0) - 2025-04-09

### Added

- allow image with changed extension ([#141](https://github.com/francisdb/vpin/pull/141))
- primitive mesh handling moved to primitive ([#134](https://github.com/francisdb/vpin/pull/134))
- introduce part_group_name and extra flasher fields ([#136](https://github.com/francisdb/vpin/pull/136))

### Fixed

- *(vpx)* truncate material name for legacy format ([#138](https://github.com/francisdb/vpin/pull/138))

### Other

- reduce image size warning logs ([#140](https://github.com/francisdb/vpin/pull/140))
- revert layer fields order change ([#139](https://github.com/francisdb/vpin/pull/139))

## [0.17.6](https://github.com/francisdb/vpin/compare/v0.17.5...v0.17.6) - 2025-03-10

### Other

- Add table ini filepath from vpx path ([#131](https://github.com/francisdb/vpin/pull/131))

## [0.17.5](https://github.com/francisdb/vpin/compare/v0.17.4...v0.17.5) - 2025-02-21

### Other

- rust edition 2024 (#130)
- Revert "build: rust edition 2024"
- rust edition 2024
- *(deps)* update fake requirement from 3.0.1 to 4.0.0 (#128)

## [0.17.4](https://github.com/francisdb/vpin/compare/v0.17.3...v0.17.4) - 2025-02-12

### Fixed

- guess image file format and fall back to extension (#127)
- *(test)* option random distribution

## [0.17.3](https://github.com/francisdb/vpin/compare/v0.17.2...v0.17.3) - 2025-01-31

### Other

- *(deps)* update nom requirement from 7.1.3 to 8.0.0 (#124)
- *(deps)* update rand requirement from 0.8.5 to 0.9.0 (#125)
- *(deps)* update dirs requirement from 5.0.1 to 6.0.0 (#122)

## [0.17.2](https://github.com/francisdb/vpin/compare/v0.17.1...v0.17.2) - 2024-12-18

### Fixed

- ls should show gameitems (#120)

## [0.17.1](https://github.com/francisdb/vpin/compare/v0.17.0...v0.17.1) - 2024-12-18

### Added

- extractvbs now can write to any vbs path (#118)

## [0.17.0](https://github.com/francisdb/vpin/compare/v0.16.1...v0.17.0) - 2024-12-05

### Other

- *(directb2s)* add missing fields ([#114](https://github.com/francisdb/vpin/pull/114))
- new clippy rules ([#115](https://github.com/francisdb/vpin/pull/115))

## [0.16.1](https://github.com/francisdb/vpin/compare/v0.16.0...v0.16.1) - 2024-11-12

### Other

- *(deps)* dependency updates that dependabot skipped

## [0.16.0](https://github.com/francisdb/vpin/compare/v0.15.6...v0.16.0) - 2024-11-12

### Other

- TonyMcMapface -> AgX ([#111](https://github.com/francisdb/vpin/pull/111))
- *(deps)* update quick-xml requirement from 0.36.0 to 0.37.0 ([#108](https://github.com/francisdb/vpin/pull/108))

## [0.15.6](https://github.com/francisdb/vpin/compare/v0.15.5...v0.15.6) - 2024-09-10

### Fixed

- *(vpx)* io error propagation for extractvbs/verify ([#105](https://github.com/francisdb/vpin/pull/105))

## [0.15.5](https://github.com/francisdb/vpin/compare/v0.15.4...v0.15.5) - 2024-08-28

### Fixed
- *(wav)* handle additional wav headers ([#103](https://github.com/francisdb/vpin/pull/103))

## [0.15.4](https://github.com/francisdb/vpin/compare/v0.15.3...v0.15.4) - 2024-08-16

### Other
- *(deps)* dependency updates that dependabot skipped ([#101](https://github.com/francisdb/vpin/pull/101))
- *(deps)* update quick-xml requirement from 0.35.0 to 0.36.0 ([#100](https://github.com/francisdb/vpin/pull/100))
- *(deps)* update quick-xml requirement from 0.34.0 to 0.35.0 ([#99](https://github.com/francisdb/vpin/pull/99))
- *(deps)* update quick-xml requirement from 0.32.0 to 0.34.0 ([#97](https://github.com/francisdb/vpin/pull/97))

## [0.15.3](https://github.com/francisdb/vpin/compare/v0.15.2...v0.15.3) - 2024-06-19

### Added
- expose gamedata to json ([#95](https://github.com/francisdb/vpin/pull/95))

### Other
- test code cleanup

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
