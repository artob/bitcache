CARGO = cargo
READMER = readmer

all: README.md

README.md: ../.config/readmer/rust/README.md.liquid
	$(READMER) render $< > $@

build: Cargo.toml
	$(CARGO) build --release

check: Cargo.toml
	$(CARGO) test -- --nocapture

clean: Cargo.toml
	$(CARGO) clean
	rm -rf *~ target

maintainer-clean: Cargo.toml
	rm -f Cargo.lock

.PHONY: all check clean maintainer-clean
.SECONDARY:
.SUFFIXES:
