READMER = readmer

all: README.md

README.md: ../.config/readmer/js/README.md.liquid
	$(READMER) render $< > $@

clean:
	rm -rf build dist npm-debug.log

maintainer-clean:
	rm -rf node_modules bun.lock package-lock.json yarn.lock

.PHONY: all clean maintainer-clean
.SECONDARY:
.SUFFIXES:
