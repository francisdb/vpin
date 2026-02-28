# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.23.2](https://github.com/francisdb/vpin/compare/v0.23.1...v0.23.2) - 2026-02-28

### Fixed

- remove unnecessary warnings for non-PCM wave formats ([#272](https://github.com/francisdb/vpin/pull/272))

## [0.23.1](https://github.com/francisdb/vpin/compare/v0.23.0...v0.23.1) - 2026-02-26

### Fixed

- correct gate timer BIFF writing methods to respect order ([#270](https://github.com/francisdb/vpin/pull/270))

## [0.23.0](https://github.com/francisdb/vpin/compare/v0.22.0...v0.23.0) - 2026-02-26

### Added

- grouping & custom props in glTF export ([#266](https://github.com/francisdb/vpin/pull/266))
- [**breaking**] json flipper return_ -> return ([#264](https://github.com/francisdb/vpin/pull/264))
- flipper texture for gltf export ([#263](https://github.com/francisdb/vpin/pull/263))
- [**breaking**] refactor timers to avoid duplication ([#261](https://github.com/francisdb/vpin/pull/261))

### Fixed

- wall smoothing ([#267](https://github.com/francisdb/vpin/pull/267))
- ensure vertex normals consistency with geometric face normals after glTF transform ([#265](https://github.com/francisdb/vpin/pull/265))

### Other

- add incandescent light example ([#269](https://github.com/francisdb/vpin/pull/269))
- lighting documentation ([#268](https://github.com/francisdb/vpin/pull/268))

## [0.22.0](https://github.com/francisdb/vpin/compare/v0.21.1...v0.22.0) - 2026-02-25

### Added

- ramp surface height interpolation
- [**breaking**] light insert mesh
- add support for exporting invisible items in glTF/GLB ([#258](https://github.com/francisdb/vpin/pull/258))
- proper translate for gltf ([#254](https://github.com/francisdb/vpin/pull/254))
- gltf export ([#251](https://github.com/francisdb/vpin/pull/251))
- better transparent pixels check

### Fixed

- ramp mesh smoothing ([#257](https://github.com/francisdb/vpin/pull/257))

### Other

- document light related fields

## [0.21.1](https://github.com/francisdb/vpin/compare/v0.21.0...v0.21.1) - 2026-02-16

### Added

- part group order warnings ([#249](https://github.com/francisdb/vpin/pull/249))

### Fixed

- update DILI handling to use quantization ([#246](https://github.com/francisdb/vpin/pull/246))

## [0.21.0](https://github.com/francisdb/vpin/compare/v0.20.15...v0.21.0) - 2026-02-14

### Added

- [**breaking**] force major version bump after adding mesh generation
- convert vpx to glb (gltf) ([#231](https://github.com/francisdb/vpin/pull/231))
- add GLTF format support for mesh extraction and reading ([#229](https://github.com/francisdb/vpin/pull/229))

### Other

- add missing ignores on integration test ([#243](https://github.com/francisdb/vpin/pull/243))
- cfb 0.14.0 ([#242](https://github.com/francisdb/vpin/pull/242))

## [0.20.15](https://github.com/francisdb/vpin/compare/v0.20.14...v0.20.15) - 2026-02-06

### Other

- log warn instead of tracing warn

## [0.20.14](https://github.com/francisdb/vpin/compare/v0.20.13...v0.20.14) - 2026-02-06

### Added

- add validation for material conversion and log discrepancies ([#224](https://github.com/francisdb/vpin/pull/224))

### Other

- update description to reflect removal of directb2s
- remove unneeded dependencies ([#227](https://github.com/francisdb/vpin/pull/227))
- directb2s code moved to separate repo ([#225](https://github.com/francisdb/vpin/pull/225))

## [0.20.13](https://github.com/francisdb/vpin/compare/v0.20.12...v0.20.13) - 2026-02-06

### Fixed

- correct playfield reflection strength quantization ([#222](https://github.com/francisdb/vpin/pull/222))

## [0.20.12](https://github.com/francisdb/vpin/compare/v0.20.11...v0.20.12) - 2026-02-06

### Fixed

- warn for invalid bool ([#220](https://github.com/francisdb/vpin/pull/220))
- *(json)* correct type fields from type_ to type ([#218](https://github.com/francisdb/vpin/pull/218))

## [0.20.11](https://github.com/francisdb/vpin/compare/v0.20.10...v0.20.11) - 2026-01-29

### Added

- improve directory listing ([#213](https://github.com/francisdb/vpin/pull/213))

### Other

- split up expanded module ([#211](https://github.com/francisdb/vpin/pull/211))

## [0.20.10](https://github.com/francisdb/vpin/compare/v0.20.9...v0.20.10) - 2026-01-28

### Other

- switch flate2 dependency to use zlib-rs feature ([#209](https://github.com/francisdb/vpin/pull/209))

## [0.20.9](https://github.com/francisdb/vpin/compare/v0.20.8...v0.20.9) - 2026-01-28

### Added

- add GLB format support for VPX extraction ([#205](https://github.com/francisdb/vpin/pull/205))
- directb2s cargo feature and update documentation ([#207](https://github.com/francisdb/vpin/pull/207))
- switch to f32 precision in OBJ file handling ([#206](https://github.com/francisdb/vpin/pull/206))

### Other

- update cfb dependency to version 0.13.0
- Revert "chore: release v0.20.9 ([#203](https://github.com/francisdb/vpin/pull/203))"
- release v0.20.9 ([#203](https://github.com/francisdb/vpin/pull/203))
- vertex compression and add tracing instrumentation ([#204](https://github.com/francisdb/vpin/pull/204))
- *(deps)* update quick-xml requirement from 0.38.4 to 0.39.0 ([#187](https://github.com/francisdb/vpin/pull/187))
- set cfb buffer sizes ([#193](https://github.com/francisdb/vpin/pull/193))

## [0.20.8](https://github.com/francisdb/vpin/compare/v0.20.7...v0.20.8) - 2026-01-20

### Other

- add support for 2 more image file formats
- running integration tests in memory ([#202](https://github.com/francisdb/vpin/pull/202))
- select specific image format features ([#200](https://github.com/francisdb/vpin/pull/200))

## [0.20.7](https://github.com/francisdb/vpin/compare/v0.20.6...v0.20.7) - 2026-01-20

### Other

- update npm package references

## [0.20.6](https://github.com/francisdb/vpin/compare/v0.20.5...v0.20.6) - 2026-01-20

### Other

- update Node.js version to 24 for OICD compatibility

## [0.20.5](https://github.com/francisdb/vpin/compare/v0.20.4...v0.20.5) - 2026-01-20

### Other

- update README with links to npm package
- allow triggering further workflow runs

## [0.20.4](https://github.com/francisdb/vpin/compare/v0.20.3...v0.20.4) - 2026-01-20

### Added

- add wasm support for web editor ([#188](https://github.com/francisdb/vpin/pull/188))

### Other

- wasm publishing to rpm repo
- *(deps)* bump actions/setup-node from 4 to 6 ([#196](https://github.com/francisdb/vpin/pull/196))
- wasm tests ([#195](https://github.com/francisdb/vpin/pull/195))

## [0.20.3](https://github.com/francisdb/vpin/compare/v0.20.2...v0.20.3) - 2026-01-14

### Added

- parallel feature (enabled by default) ([#191](https://github.com/francisdb/vpin/pull/191))

## [0.20.2](https://github.com/francisdb/vpin/compare/v0.20.1...v0.20.2) - 2026-01-14

### Other

- move fake dummy derive to dev-dependencies ([#189](https://github.com/francisdb/vpin/pull/189))

## [0.20.1](https://github.com/francisdb/vpin/compare/v0.20.0...v0.20.1) - 2026-01-02

### Other

- macro for implementing shared attributes ([#186](https://github.com/francisdb/vpin/pull/186))
- improve image dimensions warning ([#184](https://github.com/francisdb/vpin/pull/184))

## [0.20.0](https://github.com/francisdb/vpin/compare/v0.19.1...v0.20.0) - 2025-12-30

### Other

- optional editor_layer, unpadded vertex3d, optional gamedata fields ([#183](https://github.com/francisdb/vpin/pull/183))
- add support for ball element in 10.8.1 ([#181](https://github.com/francisdb/vpin/pull/181))

## [0.19.1](https://github.com/francisdb/vpin/compare/v0.19.0...v0.19.1) - 2025-12-17

### Added

- multithreaded obj writing ([#180](https://github.com/francisdb/vpin/pull/180))

### Fixed

- sanitize not symmetric ([#178](https://github.com/francisdb/vpin/pull/178))

## [0.19.0](https://github.com/francisdb/vpin/compare/v0.18.7...v0.19.0) - 2025-12-14

### Other

- add PMSK, MD5H, and CLBH. Move LAYR/LANR before GRUP. ([#174](https://github.com/francisdb/vpin/pull/174))
- replace actions/cache with Swatinem/rust-cache
- update toolchain action to dtolnay/rust-toolchain
- weekly integration test build ([#175](https://github.com/francisdb/vpin/pull/175))

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
