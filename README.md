
# Starlight

**NOTE**: At the moment Starlight is actively developed on my localhost and `dev` branch will not build and work but if you want to play with it you can try these commits:
- `2086a3f2446fe4563097918640d67bc6c7b31695`: working starlight with new Immix GC, works just fine but does not build on Windows
- `5c4205542e280dfcece3569cc31316279c603436`: the last commit known to work on Windows just fine, I recommend using it.

Starlight is a JS engine in Rust which focuses on performance rather than ensuring 100% safety of JS runtime.

[Discord](https://discord.gg/4Kjt3ZE) <- PLDev community discord, there's #starlight channel.

# Features
- Bytecode interpreter
- Mostly-precise GC
- Polymorphic inline caches
- Startup snapshots
- Executable JS bundles

# Why?

I was developing my own JS-like language but then I realized that there's no point in it and decided to try to create optimized JS engine with JIT,inline caches, fast GC and other fun stuff that is interesting me.

# Starlight VS Boa
Starlight is much faster than Boa in a lot of ways one of which is object property accesses but Starlight is designed more like JS engine that can be potentionally used in web browsers like V8 and JSC and not something small and embedabble (we have 1.6MB binary through!).

# Startup snapshots
The ES specification includes lots and lots of builtin objects. Every new single `Runtime` type instance has to set-up and initialize these builtins at the time runtime is created. It takes quite some time to do this from scratch.


To solve this problems V8-like startup snapshots were implemented. Startup snapshots just deserialize previously serialized heap state which might reduce load times singnificantly (not much difference right now since not that many builtins is implemented). Custom JS functions and objects could be added to global state and then serialized into binary file and deserialized later which removes overhead of initializing objects, parsing source code and compiling it to bytecode to just simple deserialization of raw bytes. Think of it like AOT compilation but simpler and smaller.


# Build and install instructions (Linux,*BSD,macOS)
Executing these two commands will result in building starlight and installing libraries to necessary folders: 
```sh
git clone https://github.com/starlight-js/starlight
./build.sh # use build-debug.sh for debug build
```

NOTE: macOS and FreeBSD do not use `*.so` file format so `.dynlib` files from target/release should be installed manually.

# Bundles

Starlight supports creating executable JavaScript bundles. To create one `starlight-bundle` could be used: just run it on one of JS files like `starlight-bundle file.js file-bundle` and it will produce statically linked executable file `file-bundle` which can be run. Internally bundle is just heap snapshot after JS file was compiled and simple call to execute code from this snapshot.

***NOTE*** `starlight-bundle` might panic when run on platform that does have `cc` available in `PATH` so `--output-c` option should be used and C file should be compiled and linked manually.

# Get Started
## Working with nightly Rust
```bash
# https://rust-lang.github.io/rustup/concepts/channels.html
rustup toolchain install nightly
rustup run nightly rustc --version
rustup default nightly
```

## Run Js File
```bash
cargo run --bin sl examples/hello-world.js
```
