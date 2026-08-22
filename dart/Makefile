READMER = readmer

all: README.md

README.md: ../.config/readmer/dart/README.md.liquid
	$(READMER) render $< > $@

clean:
	rm -rf .dart_tool build

maintainer-clean:
	rm -rf pubspec.lock .packages

.PHONY: all clean maintainer-clean
.SECONDARY:
.SUFFIXES:
