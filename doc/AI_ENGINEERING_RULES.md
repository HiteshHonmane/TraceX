# AI Engineering Rules

> **This document defines how AI agents contribute to TraceX.**
>
> Every AI coding agent (Cursor, Claude Code, Codex, Gemini CLI, etc.) MUST follow these rules.
>
> If any generated code violates these rules, the code MUST be rejected and regenerated.
>
> Working code is NOT enough.
>
> Correct architecture always takes priority.

---

# Mission

Every line of code should make TraceX:

- Faster
- Cleaner
- Easier to maintain
- Easier to extend

Never optimize for writing less code.

Always optimize for building a product that can survive for 10+ years.

---

# Core Principles

## Principle 1 — Read Before You Write

Before implementing ANY feature, the AI MUST read:

doc/vision.md

doc/01-Product-Requirements.md

doc/02-System-Architecture.md

If implementation conflicts with documentation,

documentation wins.

---

## Principle 2 — Never Guess

If requirements are unclear,

DO NOT invent behavior.

Instead:

- Leave a TODO
- Ask for clarification
- Explain assumptions

Never silently change product behavior.

---

## Principle 3 — Follow Existing Patterns

Never introduce a new coding style if an existing one already exists.

Consistency is more valuable than cleverness.

---

# Code Generation Rules

## Single Responsibility

Every module should do ONE thing.

Bad

Logger
Settings
Parser
Storage
Export

inside same file.

Good

logger.rs

parser.rs

storage.rs

export.rs

---

## Maximum File Size

Target:

< 400 lines

Acceptable:

600 lines

Never exceed:

1000 lines

If a file grows too large,

split it.

---

## Function Size

Target:

10–40 lines

Avoid giant functions.

If scrolling is required to understand a function,

it's too large.

---

## Naming

Names should explain intent.

Bad

mgr

helper

util

temp

Good

SessionManager

LogParser

WorkspaceStorage

TransportFactory

Never abbreviate unless universally understood.

---

# Architecture Rules

Never bypass architecture.

UI

↓

Application

↓

Core

↓

Infrastructure

Never reverse dependency direction.

---

# UI Rules

UI should only:

- Display data
- Collect user input
- Dispatch actions

UI must never:

- Parse logs
- Search logs
- Read files
- Read serial ports
- Execute business logic

---

# Transport Rules

Every transport must implement the same interface.

UART

BLE

TCP

UDP

MQTT

CAN

should behave consistently.

Never special-case UART.

---

# Event Rules

Communication should happen through events.

Bad

Parser

↓

UI

Good

Parser

↓

Event Bus

↓

Consumers

Modules should not tightly couple together.

---

# Error Handling

Never ignore errors.

Forbidden

unwrap()

expect()

panic!()

except:

- Tests
- Prototypes explicitly marked

Always propagate meaningful errors.

---

# Logging

Every important operation should emit structured logs.

Never print debugging information using println!.

Use the project's logging framework.

---

# Performance Rules

Performance is a feature.

Avoid:

Unnecessary cloning

Blocking UI

Repeated allocations

Repeated parsing

Repeated indexing

Repeated string copies

Think before allocating memory.

---

# Memory Rules

Prefer borrowing.

Clone only when ownership is required.

Avoid unnecessary heap allocation.

Large data should not be copied multiple times.

---

# Async Rules

Anything that may block must execute outside the UI thread.

Includes:

Serial IO

Disk IO

Network IO

Search indexing

Export

Compression

Never freeze the interface.

---

# Dependency Rules

Before adding any dependency ask:

Can the standard library solve this?

Can existing crates solve this?

Is this dependency actively maintained?

Is it lightweight?

Will it still be maintained in five years?

Every dependency increases maintenance cost.

---

# Code Duplication

If logic exists twice,

extract it.

Never duplicate code because it is faster to write.

---

# Public APIs

Every public type should have:

Documentation

Purpose

Examples (where appropriate)

Avoid exposing unnecessary APIs.

Everything public becomes permanent.

---

# Configuration

Never hardcode:

Paths

Ports

Timeouts

Colors

Magic numbers

Everything configurable belongs in configuration.

---

# Tests

Every feature should include tests.

At minimum:

Unit Tests

Critical business logic must be tested.

Bugs fixed should receive regression tests.

---

# Git Rules

One commit should represent one logical change.

Avoid giant commits touching unrelated systems.

Commit messages should explain WHY,

not only WHAT.

---

# Refactoring

Refactoring is encouraged.

Before adding complexity ask:

Can existing code be improved instead?

Can duplication be removed?

Can this module become simpler?

Prefer deleting code over adding code.

---

# Documentation

Every major feature should update:

Architecture

Documentation

Public API docs

Never let documentation become outdated.

---

# Security

Never expose secrets.

Never hardcode credentials.

Validate external input.

Assume every external input is malformed.

---

# Cross Platform

Never introduce platform-specific code unless unavoidable.

Windows

Linux

macOS

should remain first-class citizens.

---

# Review Checklist

Before considering a task complete:

[ ] Architecture respected

[ ] No duplicated code

[ ] No UI business logic

[ ] Performance considered

[ ] Tests added

[ ] Documentation updated

[ ] Public APIs documented

[ ] No unnecessary dependencies

[ ] Naming is clear

[ ] Errors handled properly

---

# AI Behavior

An AI engineer should think like a senior software engineer.

Never optimize for:

- Speed of generation
- Fewer tokens
- Clever tricks

Always optimize for:

- Maintainability
- Readability
- Extensibility
- Performance
- Simplicity

When uncertain,

choose the simpler architecture.

---

# Engineering Philosophy

Code is not the product.

Architecture is not the product.

The developer experience is the product.

Every decision should answer one question:

> Does this help developers find problems faster?

If the answer is no,

it probably does not belong in TraceX.

---

# Final Rule

Write code that another engineer can understand in six months without asking questions.

Future maintainability is more important than present convenience.
