# Runner

Standalone executable for running fuzzers inside isolated container. It sets cwd, environment variables, command line arguments, e.t.c, and gathers results from stdout/stderr output streams. Written in Rust

# Build & run

```bash
cargo build
cargo run -- config.json
```

# Production build

This build must be used in production:
- Statically linked, no dynamic dependencies
- Size must be less than 1MB to fit into etcd
- Must be mounted into target container

```bash
RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-gnu
upx -9 -o runner ./target/x86_64-unknown-linux-gnu/release/runner
```