# Activate AI Fellows Submission

## Project

**Imprint** is a summonable browser overlay agent with local user memory.

It sits on top of your existing workflow, understands the page you are currently on, remembers stable preferences through a local `user.md` file, and can take lightweight browser actions with user approval.

This is intentionally **not** a general-purpose desktop agent, and not a full browser replacement. The goal is to build the smallest version of an agent that feels genuinely useful: one that understands the current page, remembers who you are, and can help you complete web tasks without forcing you into a separate chat workflow.

## Core Idea

Most AI assistants today fall into one of two categories:

- they are strong at chat, but disconnected from the page you are actively using
- they can automate tasks, but they feel broad, heavy, and difficult to trust

This project focuses on a narrower and more usable interaction model:

- invoke an overlay with a shortcut
- ask for help about the current page
- let the assistant inspect page context
- optionally approve one or two actions
- reuse local memory so the assistant behaves more like **your** agent over time

## Why This Exists

I wanted something that feels faster and lighter than opening a separate AI product, copying context into it, and then manually applying the result back into the browser.

The product thesis is:

> **A summonable browser-aware overlay agent that understands the page in front of you and uses local memory to help you complete tasks with less friction.**

## What Makes It Interesting

### 1. Overlay-first interaction

The assistant is not a separate destination. It is summoned over your current workflow.

That makes the interaction feel immediate:

- no tab switching
- no copy-pasting page context into another tool
- no need to treat the assistant like a separate app

### 2. Page-aware execution

The assistant is designed around the current browser page.

It can:

- inspect page state
- summarize what is on the page
- extract relevant information
- identify interactive elements
- take lightweight actions such as click, type, and navigate

The emphasis is not on broad autonomy. The emphasis is on **useful page-local help**.

### 3. Local memory with user ownership

The agent keeps memory in a local `user.md` file.

That memory is:

- local
- visible
- editable
- portable
- downloadable

The purpose of this file is to store stable, high-value context such as:

- user preferences
- writing style
- recurring workflows
- preferred tools or sites
- ongoing goals that matter across sessions

This avoids the black-box feeling of hidden memory systems while still giving the agent continuity.

### 4. Approval before action and before memory changes

The system is designed to ask before doing meaningful things.

That includes:

- risky browser actions
- proposed memory updates

This makes the agent feel collaborative rather than uncontrollable.

## Smallest Useful Version

The intentionally scoped version of this product includes:

- a global shortcut to open the overlay
- reading the current browser page state
- answering questions about the current page
- taking a few browser actions with approval
- reading from local `user.md` memory
- proposing updates to `user.md`

What it intentionally leaves out:

- full desktop control
- raw mouse/vision loops
- plugin ecosystems
- voice input
- multi-surface automation across browser, shell, and OS all at once
- broad “do everything” agent claims

The point is to keep the product understandable and demonstrable in a few minutes.

## Example Use Cases

- “Summarize this page in 5 bullets using my preferred concise style.”
- “Fill this form using my saved preferences and stop before submitting.”
- “Compare the options on this page and recommend one for my role and budget.”
- “Remember that I prefer short bullet answers and save that to memory.”
- “Use my memory to draft this outreach reply in my usual tone.”

## Why This Is Different

This is not trying to be the biggest agent.

It is trying to be the **most usable narrow agent** for a common situation:

> you are already on a web page, you need help right now, and you want the assistant to understand both the page and your preferences.

The difference is not novelty for novelty’s sake. The difference is product shape:

- overlay, not destination app
- page-aware, not generic chat-only
- local memory, not opaque remote memory
- action with approval, not uncontrolled autonomy

## Design Judgment

The full underlying prototype explored many more capabilities, including broader desktop automation. For this submission, the better judgment is to keep only the parts that strengthen the core idea.

That means prioritizing:

- immediacy
- legibility
- trust
- focused usefulness

and cutting anything that makes the product harder to explain or less reliable.

## What Success Looks Like

A successful version of this product should make a reviewer feel:

- “I understand exactly what this is.”
- “I can see myself using this.”
- “This feels smaller and sharper than a generic agent wrapper.”
- “The memory model is actually thoughtful.”

## In One Sentence

**Imprint is a summonable browser overlay agent that understands the page you are on, remembers your stable preferences through local memory, and helps you take lightweight actions with approval.**

