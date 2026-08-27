# See: https://github.com/asciinema/agg

AGG = agg --quiet
AGGFLAGS = --cols 100 --rows 30 --theme dracula --renderer swash --idle-time-limit 10

all: install.gif

install.gif: install.cast
	$(AGG) $(AGGFLAGS) $< $@

clean:
	rm -f *~ *.gif

maintainer-clean: clean
	rm -f *.cast

.PHONY: all clean maintainer-clean
.SECONDARY:
.SUFFIXES:
