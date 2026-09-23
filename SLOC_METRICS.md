# SLOC Metrics

Generated with `tokei 15.0.0` on 2026-09-23.

The counts below use `tokei`'s default ignore behavior, so ignored paths such as `target/`, `.tools/`, `data/`, `.test-db/`, and `yourls-urls.txt` are excluded by the repository `.gitignore`. The root report command also excludes this file so the project-wide totals remain reproducible after the report is committed.

## Workspace Summary

| Scope | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Project total | 24 | 2,139 | 1,576 | 269 | 294 |

## Crate Summary

| Crate | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| `zhorten-app` | 3 | 501 | 423 | 34 | 44 |
| `zhorten-core` | 4 | 168 | 111 | 25 | 32 |
| `zhorten-service` | 3 | 285 | 229 | 18 | 38 |
| `zhorten-server` | 7 | 745 | 575 | 82 | 88 |

## Project Language Breakdown

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
| Rust | 9 | 1,419 | 1,255 | 28 | 136 |
| SVG | 1 | 4 | 4 | 0 | 0 |
| TOML | 5 | 103 | 90 | 0 | 13 |
| Total | 24 | 2,139 | 1,576 | 269 | 294 |

## Crate Language Details

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
| Rust | 2 | 124 | 103 | 0 | 21 |
| TOML | 1 | 10 | 8 | 0 | 2 |
| Total | 4 | 168 | 111 | 25 | 32 |

### `zhorten-service`

| Language | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Markdown | 1 | 27 | 0 | 17 | 10 |
| Markdown embedded in Rust doc comments | 1 | 1 | 0 | 1 | 0 |
| Rust | 1 | 246 | 220 | 0 | 26 |
| TOML | 1 | 11 | 9 | 0 | 2 |
| Total | 3 | 285 | 229 | 18 | 38 |

### `zhorten-server`

| Language | Files | Lines | Code | Comments | Blanks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Bash embedded in Markdown | 1 | 3 | 3 | 0 | 0 |
| Markdown | 1 | 44 | 0 | 28 | 16 |
| Markdown embedded in Rust doc comments | 5 | 39 | 0 | 34 | 5 |
| Rust | 5 | 633 | 548 | 20 | 65 |
| TOML | 1 | 26 | 24 | 0 | 2 |
| Total | 7 | 745 | 575 | 82 | 88 |

## Commands

```powershell
tokei --exclude SLOC_METRICS.md .
tokei crates/zhorten-app
tokei crates/zhorten-core
tokei crates/zhorten-service
tokei crates/zhorten-server
```
