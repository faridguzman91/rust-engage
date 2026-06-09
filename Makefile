# @faridguzman: Makefile for the engage project.
# Runs the relay server and Tauri desktop client simultaneously.
#
# Usage:
#   make dev            — start server + client in parallel (recommended)
#   make server         — start relay server only
#   make client         — start Tauri client only
#   make install        — install frontend dependencies
#   make build          — production build (client binary + bundled server)
#   make android-init   — generate src-tauri/gen/android/ (run once after SDK setup)
#   make android-dev    — live-reload dev build on connected Android device/emulator
#   make android-build  — release APK in src-tauri/gen/android/app/build/outputs/apk/
#   make docker-up      — start server via Docker Compose
#   make docker-down    — stop Docker Compose services
#   make clean          — remove build artefacts

SHELL       := /bin/bash
SERVER_DIR  := ../engage-server
PNPM        := pnpm

# Build on local disk — avoids macOS ._* resource-fork files on external volumes
CARGO_TARGET_DIR := $(HOME)/.cargo-targets/engage
export CARGO_TARGET_DIR

# If nvm is present, prepend the latest Node 22 bin dir so make doesn't fall
# back to the system Node (which may be too old for pnpm 9).
NVM_NODE22 := $(shell ls -d $(HOME)/.nvm/versions/node/v22.* 2>/dev/null | sort -V | tail -1)
ifneq ($(NVM_NODE22),)
  export PATH := $(NVM_NODE22)/bin:$(PATH)
endif

.PHONY: dev server client install build android-init android-dev android-build docker-up docker-down clean help

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

## Generate the Android Gradle project (run once; requires Android SDK + NDK)
android-init:
	$(PNPM) tauri android init

## Start dev build on connected Android device or running emulator
android-dev:
	$(PNPM) tauri android dev

## Build a release APK — output: src-tauri/gen/android/app/build/outputs/apk/
android-build: install
	$(PNPM) tauri android build --apk

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
	rm -rf dist node_modules $(CARGO_TARGET_DIR)

help:
	@grep -E '^##' Makefile | sed 's/## /  /'
