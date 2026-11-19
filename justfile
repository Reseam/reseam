target := env("CARGO_TARGET_DIR", "target")

[parallel]
check: rust browser

generate:
    cargo xtask regen all
    cargo xtask runtime
    ./gradlew spotlessCheck :reseam-sdk:compileKotlinJvm :reseam-patch-sdk:checkKotlinAbi :reseam-patch-sdk:test :reseam-gradle-plugin:test

rust: generate
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace --no-fail-fast

browser: generate
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
    install -D {{ target }}/release/reseam dist/reseam-linux-x64
    install -D {{ target }}/x86_64-pc-windows-gnullvm/release/reseam.exe dist/reseam-windows-x64.exe
    cargo xtask prune
    ./gradlew assemble
    ./gradlew publishAllPublicationsToForgejoRepository :app-runtime:reseam-app-runtime:publishAllPublicationsToForgejoRepository -PreseamSdkVersion={{ tag }}
    bun run --cwd browser types
    cd browser && npm version --no-git-tag-version --allow-same-version {{ trim_start_match(tag, "v") }}
    cd browser && npm config set //git.reseam.app/api/packages/reseam/npm/:_authToken "$FORGEJO_PACKAGES_TOKEN" && npm publish
    dist/reseam-linux-x64 release files dist --tag {{ tag }} --to s3://reseam-releases/engine --keep 3 --forge-release
    # The GitHub mirror gets the same files, as a second place to download them from.
    git push "https://x-access-token:$MIRROR_RELEASE_TOKEN@github.com/Reseam/reseam.git" refs/tags/{{ tag }}
    GITHUB_SERVER_URL=https://github.com GITHUB_API_URL=https://api.github.com RELEASE_TOKEN="$MIRROR_RELEASE_TOKEN" \
        dist/reseam-linux-x64 release files dist --tag {{ tag }}
