READMER = readmer

all: README.md

README.md: .config/readmer/README.md.liquid
	$(READMER) render $< > $@

readmes: README.md
	@rc=0; \
	for dir in dart js python ruby rust; do \
		if [ -d $$dir ]; then \
			$(MAKE) -C $$dir README.md || rc=$$?; \
		fi; \
	done; \
	exit $$rc

clean:
	@rc=0; \
	for dir in dart js python ruby rust; do \
		if [ -d $$dir ]; then \
			$(MAKE) -C $$dir clean || rc=$$?; \
		fi; \
	done; \
	exit $$rc

maintainer-clean:
	@rc=0; \
	for dir in dart js python ruby rust; do \
		if [ -d $$dir ]; then \
			$(MAKE) -C $$dir maintainer-clean || rc=$$?; \
		fi; \
	done; \
	exit $$rc

.PHONY: all readmes clean maintainer-clean
.SECONDARY:
.SUFFIXES:
