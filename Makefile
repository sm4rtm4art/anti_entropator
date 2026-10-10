SHELL := /bin/bash

DOWNLOADS ?= $(HOME)/Downloads
# Extra CLI flags, e.g. ARGS="--source downloads --include '*.pdf'"
ARGS ?=
# Iceberg table is file_catalog. `files` is a CLI alias `query` registers for
# iceberg.anti_entropator.file_catalog (see src/query/mod.rs).
QUERY ?= SELECT * FROM files LIMIT 10

# The CLI reads credentials from the environment only; export .env for it.
WITH_ENV := set -a; if [ -f .env ]; then . ./.env; fi; set +a;

TARGETS := help setup up down doctor init profile scan ingest-dry-run ingest query fmt clippy test check docs-shell
.PHONY: $(TARGETS)

# `make scan ~/Documents` would treat the path as a target and silently scan
# $(DOWNLOADS); fail instead.
UNKNOWN_GOALS := $(filter-out $(TARGETS),$(MAKECMDGOALS))
ifneq ($(UNKNOWN_GOALS),)
$(error Unknown target(s): $(UNKNOWN_GOALS). Pass a folder as DOWNLOADS=<path> and CLI flags as ARGS="...")
endif

help:
	@echo "Anti-Entropator local commands"
	@echo ""
	@echo "Setup and stack:"
	@echo "  make setup          Prepare .env and local data directories"
	@echo "  make up             Start the local Docker Compose stack"
	@echo "  make down           Stop the local Docker Compose stack"
	@echo "  make doctor         Run preflight checks"
	@echo "  make init           Initialize bucket, warehouse, namespace, and table"
	@echo ""
	@echo "Local file workflows:"
	@echo "  make profile        Profile DOWNLOADS=$(DOWNLOADS)"
	@echo "  make scan           Dry-run scan DOWNLOADS=$(DOWNLOADS)"
	@echo "  make ingest-dry-run Preview ingest DOWNLOADS=$(DOWNLOADS)"
	@echo "  make ingest         Ingest DOWNLOADS=$(DOWNLOADS)"
	@echo "  make query          Run QUERY='$(QUERY)'"
	@echo ""
	@echo "  Folder and flags are variables, not arguments:"
	@echo "    make ingest DOWNLOADS=~/Desktop ARGS=\"--source desktop --include '*.pdf'\""
	@echo "  CLI targets export .env automatically."
	@echo ""
	@echo "Quality checks:"
	@echo "  make check          Run fmt, clippy, and tests"
	@echo "  make docs-shell     Run docs/shell quality checks"

setup:
	@test -f .env || cp env.example .env
	@mkdir -p logs/rustfs data/postgres
	@chown -R 10001:10001 logs/rustfs || true
	@echo "Prepared local directories."
	@echo "Review .env and replace all CHANGE_ME values before starting services."

up:
	docker compose up -d

down:
	docker compose down

doctor:
	@$(WITH_ENV) cargo run -- doctor $(ARGS)

init:
	@$(WITH_ENV) cargo run -- init $(ARGS)

profile:
	cargo run --release -- profile "$(DOWNLOADS)" $(ARGS)

scan:
	@$(WITH_ENV) cargo run -- scan "$(DOWNLOADS)" --dry-run $(ARGS)

ingest-dry-run:
	@$(WITH_ENV) cargo run -- ingest "$(DOWNLOADS)" --dry-run $(ARGS)

ingest:
	@$(WITH_ENV) cargo run -- ingest "$(DOWNLOADS)" $(ARGS)

query:
	@$(WITH_ENV) cargo run -- query "$(QUERY)" $(ARGS)

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --all-features

check: fmt clippy test

docs-shell:
	typos --config .config/lint/typos.toml
	markdownlint-cli2 --config .config/lint/markdownlint-cli2.yaml
	shellcheck scripts/*.sh scripts/hooks/*
	shfmt -i 4 -d scripts/ scripts/hooks/
