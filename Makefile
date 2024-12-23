.PHONY: universal-binary clean

# Gets artifact name from cargo
ARTIFACT := $(shell cargo metadata --no-deps --format-version 1 | jq -r '.packages[].targets[] | select( .kind | map(. == "bin") | any ) | .name' )

make-mac-binary:
	echo "Making universal mac binary for $(ARTIFACT)..."
	cargo build --release --target aarch64-apple-darwin
	cargo build --release --target x86_64-apple-darwin

make-linux-binary:
	CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
	cargo build --release --target=x86_64-unknown-linux-gnu

link-binary: make-mac-binary
	lipo -create -output $(ARTIFACT) target/aarch64-apple-darwin/release/$(ARTIFACT) target/x86_64-apple-darwin/release/$(ARTIFACT)

universal-binary: link-binary

clean:
	cargo clean
