# typescript-rs

A Rust port of TypeScript-Go. Made with GPT-5.5, work in progress.

## Run

```sh
cargo run -p ts-cli -- <compiler args>
```

## Benchmark

Checker run on [Zod](https://github.com/colinhacks/zod).

| Tool | Version | Command | Median | Best |
| --- | --- | --- | ---: | ---: |
| typescript-rs | 7.0.0-dev | `<repo>/target/release/ts-cli --noEmit` | 2.76s | 2.72s |
| typescript-go | 7.0.0-dev.20260615.1 | `bunx tsgo --noEmit` | 1.64s | 1.57s |
| TypeScript | 5.5.4 | `bunx typescript --noEmit` | 7.73s | 7.69s |
