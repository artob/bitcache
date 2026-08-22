READMER = readmer

all: README.md

README.md: ../.config/readmer/python/README.md.liquid
	$(READMER) render $< > $@

clean:
	rm -rf build dist *.egg-info .pytest_cache .mypy_cache __pycache__
	find . -name '*.pyc' -delete || true

maintainer-clean:
	rm -rf .venv Pipfile.lock poetry.lock

.PHONY: all clean maintainer-clean
.SECONDARY:
.SUFFIXES:
