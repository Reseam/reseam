export CARGO_BUILD_JOBS := env("CARGO_BUILD_JOBS", "2")
export GRADLE_OPTS := "-Xmx1g"

gradle := "./gradlew --no-daemon -Dorg.gradle.jvmargs=-Xmx1536m"

check:
    cargo xtask regen all
    cargo xtask runtime
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    {{ gradle }} spotlessCheck :reseam-sdk:compileKotlinJvm :reseam-patch-sdk:checkKotlinAbi :reseam-gradle-plugin:test
    cargo test --workspace --no-fail-fast
    bun install --cwd browser --frozen-lockfile
    bun run --cwd browser engine
    CC_wasm32_wasip1="$WASI_SDK_PATH/bin/clang" AR_wasm32_wasip1="$WASI_SDK_PATH/bin/llvm-ar" cargo clippy -p reseam-sdk-browser --target wasm32-wasip1 -- -D warnings
    bun run --cwd browser check
    bun run --cwd browser test

release tag:
    cargo xtask check-tag {{ tag }}
    cargo xtask pack-sdk
    cargo build --release -p reseam-cli
    cargo build --release -p reseam-cli --target x86_64-pc-windows-gnullvm
    {{ gradle }} assemble
    {{ gradle }} publishAllPublicationsToForgejoRepository :app-runtime:reseam-app-runtime:publishAllPublicationsToForgejoRepository -PreseamSdkVersion={{ tag }}
    bun run --cwd browser types
    cd browser && npm version --no-git-tag-version --allow-same-version {{ trim_start_match(tag, "v") }}
    cd browser && npm config set //git.reseam.app/api/packages/reseam/npm/:_authToken "$FORGEJO_PACKAGES_TOKEN" && npm publish
    install -D target/release/reseam dist/reseam-linux-x64
    install -D target/x86_64-pc-windows-gnullvm/release/reseam.exe dist/reseam-windows-x64.exe
    target/release/reseam release files dist --tag {{ tag }} --to s3://reseam-releases/engine --forge-release
