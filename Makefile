SHELL := /bin/bash

DOWNLOADS ?= $(HOME)/Downloads
# Iceberg table is file_catalog. `files` is CLI sugar rewritten by `query` to
# iceberg.anti_entropator.file_catalog (see src/query/mod.rs).
QUERY ?= SELECT * FROM files LIMIT 10

.PHONY: help setup up down doctor init profile scan ingest-dry-run ingest query fmt clippy test check docs-shell

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
	@echo "Quality checks:"
	@echo "  make check          Run fmt, clippy, and tests"
	@echo "  make docs-shell     Run docs/shell quality checks"

setup:
	@test -f .env || cp env.example .env
	@mkdir -p data/rustfs logs/rustfs data/postgres
	@chown -R 10001:10001 data/rustfs logs/rustfs || true
	@echo "Prepared local directories."
	@echo "Review .env and replace all CHANGE_ME values before starting services."

up:
	docker compose up -d

down:
	docker compose down

doctor:
	cargo run -- doctor

init:
	cargo run -- init

profile:
	cargo run --release -- profile "$(DOWNLOADS)"

scan:
	cargo run -- scan "$(DOWNLOADS)" --dry-run

ingest-dry-run:
	cargo run -- ingest "$(DOWNLOADS)" --dry-run

ingest:
	cargo run -- ingest "$(DOWNLOADS)"

query:
	cargo run -- query "$(QUERY)"

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --all-features

check: fmt clippy test

docs-shell:
	typos --config .config/lint/typos.toml
	markdownlint-cli2 --config .config/lint/markdownlint-cli2.yaml "README.md" "AGENTS.md" "CHANGELOG.md" "docs/**/*.md" "scripts/**/*.md" ".cursor/rules/**/*.mdc"
	shellcheck scripts/*.sh scripts/hooks/*
	shfmt -i 4 -d scripts/ scripts/hooks/
