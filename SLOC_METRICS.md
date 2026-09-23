# SLOC Metrics

Generated with `tokei 15.0.0` on 2026-09-23.

The implementation counts below use `tokei`'s default ignore behavior plus `--exclude "*_tests.rs"`, so ignored paths such as `target/`, `.tools/`, `data/`, `.test-db/`, and `yourls-urls.txt` are excluded by the repository `.gitignore`, and separated unit-test files are excluded from executable implementation SLOC. The root implementation command also excludes this report so the project-wide totals remain reproducible after the report is committed.

## Implementation Summary

| Scope | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Project implementation | 24 | 1,901 | 1,360 | 269 | 272 |

## Implementation By Crate

| Crate | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| `zhorten-app` | 3 | 501 | 423 | 34 | 44 |
| `zhorten-core` | 4 | 151 | 96 | 25 | 30 |
| `zhorten-service` | 3 | 139 | 96 | 18 | 25 |
| `zhorten-server` | 7 | 670 | 507 | 82 | 81 |

## Test Summary

| Scope | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Separated Rust unit tests | 4 | 237 | 215 | 0 | 22 |

## Tests By Crate

| Crate | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| `zhorten-app` | 0 | 0 | 0 | 0 | 0 |
| `zhorten-core` | 1 | 17 | 15 | 0 | 2 |
| `zhorten-service` | 1 | 145 | 132 | 0 | 13 |
| `zhorten-server` | 2 | 75 | 68 | 0 | 7 |

## Implementation Language Breakdown

| Language | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Bash embedded in Markdown | 3 | 87 | 78 | 0 | 9 |
| CSS | 1 | 80 | 80 | 0 | 0 |
| Dockerfile | 1 | 43 | 35 | 1 | 7 |
| HTML | 1 | 13 | 13 | 0 | 0 |
| INI embedded in Markdown | 1 | 21 | 19 | 0 | 2 |
| JavaScript | 1 | 3 | 2 | 0 | 1 |
| Markdown | 5 | 307 | 0 | 188 | 119 |
| Markdown embedded in Rust doc comments | 9 | 59 | 0 | 52 | 7 |
| Rust | 9 | 1,181 | 1,039 | 28 | 114 |
| SVG | 1 | 4 | 4 | 0 | 0 |
| TOML | 5 | 103 | 90 | 0 | 13 |
| Total | 24 | 1,901 | 1,360 | 269 | 272 |

## Implementation Details By Crate

### `zhorten-app`

| Language | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Bash embedded in Markdown | 1 | 11 | 10 | 0 | 1 |
| Markdown | 1 | 33 | 0 | 19 | 14 |
| Markdown embedded in Rust doc comments | 1 | 8 | 0 | 7 | 1 |
| Rust | 1 | 416 | 384 | 8 | 24 |
| TOML | 1 | 33 | 29 | 0 | 4 |
| Total | 3 | 501 | 423 | 34 | 44 |

### `zhorten-core`

| Language | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Markdown | 1 | 23 | 0 | 15 | 8 |
| Markdown embedded in Rust doc comments | 2 | 11 | 0 | 10 | 1 |
| Rust | 2 | 107 | 88 | 0 | 19 |
| TOML | 1 | 10 | 8 | 0 | 2 |
| Total | 4 | 151 | 96 | 25 | 30 |

### `zhorten-service`

| Language | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Markdown | 1 | 27 | 0 | 17 | 10 |
| Markdown embedded in Rust doc comments | 1 | 1 | 0 | 1 | 0 |
| Rust | 1 | 100 | 87 | 0 | 13 |
| TOML | 1 | 11 | 9 | 0 | 2 |
| Total | 3 | 139 | 96 | 18 | 25 |

### `zhorten-server`

| Language | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Bash embedded in Markdown | 1 | 3 | 3 | 0 | 0 |
| Markdown | 1 | 44 | 0 | 28 | 16 |
| Markdown embedded in Rust doc comments | 5 | 39 | 0 | 34 | 5 |
| Rust | 5 | 558 | 480 | 20 | 58 |
| TOML | 1 | 26 | 24 | 0 | 2 |
| Total | 7 | 670 | 507 | 82 | 81 |

## Commands

```powershell
tokei --exclude SLOC_METRICS.md --exclude "*_tests.rs" .
tokei --exclude "*_tests.rs" crates/zhorten-app
tokei --exclude "*_tests.rs" crates/zhorten-core
tokei --exclude "*_tests.rs" crates/zhorten-service
tokei --exclude "*_tests.rs" crates/zhorten-server
tokei crates/zhorten-core/src/lib_tests.rs crates/zhorten-service/src/lib_tests.rs crates/zhorten-server/src/auth_tests.rs crates/zhorten-server/src/db_tests.rs
```
