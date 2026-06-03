# @faridguzman91: Makefile for the engage project.
# Runs the relay server and Tauri desktop client simultaneously.
#
# Usage:
#   make dev        — start server + client in parallel (recommended)
#   make server     — start relay server only
#   make client     — start Tauri client only
#   make install    — install frontend dependencies
#   make build      — production build (client binary + bundled server)
#   make docker-up  — start server via Docker Compose
#   make docker-down — stop Docker Compose services
#   make clean      — remove build artefacts

SERVER_DIR := ../engage-server
PNPM       := pnpm

.PHONY: dev server client install build docker-up docker-down clean help

## Start relay server + Tauri client in parallel
dev: install
	@echo "Starting engage-server and engage client..."
	@$(MAKE) server & SERVER_PID=$$!; \
	sleep 2 && $(MAKE) client; \
	kill $$SERVER_PID 2>/dev/null; wait

## Start relay server only (reads .env from engage-server/)
server:
	@echo "Starting engage-server on :3000..."
	@cd $(SERVER_DIR) && cargo run

## Start Tauri dev client only
client:
	@echo "Starting Tauri client..."
	$(PNPM) tauri dev

## Install/update frontend dependencies
install:
	$(PNPM) install

## Production build — Tauri binary in src-tauri/target/release/bundle/
build: install
	$(PNPM) tauri build

## Start server via Docker Compose
docker-up:
	docker compose up -d
	@echo "Server running at http://localhost:3000"

## Stop Docker Compose services
docker-down:
	docker compose down

## Remove Rust + frontend build artefacts
clean:
	cargo clean --manifest-path src-tauri/Cargo.toml
	cargo clean --manifest-path $(SERVER_DIR)/Cargo.toml
	rm -rf dist node_modules

help:
	@grep -E '^##' Makefile | sed 's/## /  /'
