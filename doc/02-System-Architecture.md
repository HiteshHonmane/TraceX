# TraceX System Architecture

> This document is the engineering constitution of TraceX.
>
> Every contributor—human or AI—MUST follow the rules defined here.
> Convenience is never a reason to violate architecture.
>
> If code conflicts with this document, the code is wrong.

---

# Purpose

TraceX is being designed as a long-term developer platform.

Not a serial terminal.

Not a logging application.

A platform.

Every architectural decision must support future growth without requiring
large-scale rewrites.

---

# Core Engineering Principles

## Rule 1 — Performance First

Performance is a feature.

Never trade performance for convenience unless explicitly approved.

Goals:

- Instant UI
- Low memory usage
- High throughput
- Fast startup
- Smooth scrolling
- Large log support
- Long-running stability

---

## Rule 2 — Native First

TraceX is a native desktop application.

Do NOT introduce technologies that wrap a browser.

Examples NOT allowed:

- Electron
- Tauri Web UI
- React
- HTML rendering

Everything should be native Rust.

---

## Rule 3 — UI Never Owns Business Logic

The UI exists only to present data.

The UI MUST NOT:

- Parse logs
- Read serial ports
- Search data
- Filter data
- Save files
- Manage sessions

The UI asks the application layer.

Nothing more.

---

## Rule 4 — Everything Has One Responsibility

Every crate exists for one purpose.

If a crate starts doing unrelated work,
split it.

---

## Rule 5 — No God Objects

Forbidden:

AppManager

SystemManager

EverythingManager

Utils.rs with hundreds of unrelated functions

Large singleton objects

If a file becomes difficult to understand,
it is too large.

---

## Rule 6 — Dependency Direction

Dependencies always flow inward.

UI

↓

Application

↓

Core

↓

Traits

↓

Implementations

Never reverse this.

---

## Rule 7 — Async by Design

Anything that may block must NOT execute on the UI thread.

Examples:

- Serial read
- File write
- Parsing
- Search
- Export
- Index building

---

## Rule 8 — Extensibility

Every major subsystem should be replaceable.

Examples:

UART today

BLE tomorrow

No application code should require modification.

---

# Cargo Workspace

The project should be organized as independent crates.

tracex-app

Application entry point

tracex-ui

Desktop interface

tracex-core

Core shared types

tracex-events

Internal event bus

tracex-workspace

Workspace management

tracex-session

Active debugging sessions

tracex-transport

UART, BLE, TCP, MQTT...

tracex-parser

Convert raw bytes into structured events

tracex-log

Log engine

tracex-filter

Filtering engine

tracex-search

Search engine

tracex-storage

Persistence

tracex-settings

Configuration

tracex-plugin

Plugin framework

tracex-cli

CLI interface

tracex-cloud

Future cloud integration

---

# High Level Architecture

                 UI

                  │

        Application Layer

                  │

          Workspace Engine

                  │

            Session Manager

                  │

              Event Bus

 ┌────────────────────────────────────┐

 Transport

 Parser

 Log Engine

 Filter Engine

 Search Engine

 Storage

 Plugin Manager

 └────────────────────────────────────┘

---

# Data Flow

Device

↓

Transport

↓

Raw Bytes

↓

Parser

↓

Structured Log Event

↓

Event Bus

↓

Storage

↓

Search Index

↓

Filter Engine

↓

UI

No module should skip this pipeline.

---

# Event Driven Design

Modules communicate through events.

Modules SHOULD NOT directly call each other whenever events are sufficient.

Good:

Parser

↓

Event

↓

Storage

↓

Event

↓

Search

↓

Event

↓

UI

Bad:

Parser directly updates UI.

Parser directly saves database.

Parser directly controls filters.

---

# Storage

Workspace metadata

SQLite

Settings

TOML

Logs

Append-only binary format

Exports

TXT

CSV

JSON

Never store continuously growing logs inside SQLite.

---

# Transport Layer

Transport is an abstraction.

Supported implementations may include:

UART

BLE

TCP

UDP

MQTT

CAN

JTAG

SSH

Serial is NOT special.

It is simply one transport.

---

# Threading

UI Thread

↓

Event Dispatcher

↓

Reader Thread

↓

Parser Thread

↓

Indexer Thread

↓

Storage Thread

↓

Exporter Thread

Never block the UI.

---

# Plugin Philosophy

Plugins extend TraceX.

Plugins MUST NOT modify core architecture.

Plugins should communicate through stable APIs.

---

# AI Contributor Rules

These rules apply to every AI coding agent.

Never introduce unnecessary dependencies.

Never duplicate logic.

Prefer composition over inheritance.

Prefer traits over massive conditionals.

Never place business logic inside UI code.

Never bypass the event bus.

Never optimize prematurely.

Never create global mutable state.

Never add a dependency without clear justification.

Every public function should have a clear purpose.

Every module should have one responsibility.

Every crate should compile independently.

---

# Refactoring Rules

Refactoring is encouraged.

Breaking architecture is forbidden.

Before adding code ask:

Can this be implemented inside an existing module?

Does this violate Single Responsibility?

Will this increase coupling?

Can another module reuse this?

Does this belong in Core instead?

If uncertain,

stop and redesign.

---

# Future Vision

This architecture must support:

Multiple transports

Multiple devices

Cloud collaboration

AI diagnostics

Remote debugging

Plugin ecosystem

Enterprise features

without requiring architectural rewrites.

If a feature cannot be added without redesigning the architecture,
the architecture should be improved before implementation.

---

# Definition of Done

A feature is NOT complete because it works.

It is complete only if:

✓ Follows architecture

✓ Has tests

✓ Is documented

✓ Does not increase coupling

✓ Does not duplicate code

✓ Maintains performance

✓ Is future extensible

---

# Engineering Motto

> Build it once.
>
> Build it correctly.
>
> Build it so it never needs to be rewritten.