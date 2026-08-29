# Imprint

Imprint is a local student agent that helps turn scattered academic work into one guided workflow.

Instead of making students jump between Gmail, Google Classroom, Calendar, forms, browser tabs, and documents, Imprint keeps that context in one place and helps them take the next step. It is designed to feel less like a chatbot and more like a focused academic copilot that understands what is pending, what matters most, and what action should happen next.

## Tagline

**A stateful student agent for assignments, deadlines, emails, and browser tasks.**

## Why I Made This

Student work is fragmented.

- assignment instructions live in Classroom
- deadline changes arrive in email
- reminders sit in Calendar
- important opportunities like placements or internships often get buried
- the actual work still happens in the browser and in documents

Most assistants only help with the page in front of you. They do not really remember the student, their priorities, or the work already in progress.

I built Imprint to explore a better model: an assistant that keeps local context about the student and uses that state to help them act, not just answer.

## What It Is

Imprint is a desktop student agent with three core ideas:

- it connects to the student tools they already use
- it keeps useful profile and task context locally on the device
- it can take action in the browser when a task needs execution

The result is a product that can help a student understand what is pending, what to focus on, and how to move a task forward.

## What It Can Do

- connect to Google Workspace tools a student already uses
- read recent Gmail, Google Classroom, and Calendar context
- keep a local student profile and memory on-device
- answer questions using that live academic context
- prioritize what matters based on the student's goals and pending work
- open and navigate browser flows when action is required
- create working documents for assignment-related tasks

## Core Use Case

A student opens Imprint and asks:

**"What should I focus on right now?"**

Imprint can look across recent emails, pending coursework, deadlines, and the student's saved profile, then surface the most relevant next task.

From there, the student can ask it to continue:

- open the relevant assignment
- gather the related context
- prepare a draft
- help with a form or browser task

That is the main idea behind the product: not just answering a prompt, but helping carry the task forward.

## What Makes It Different

Most assistants are stateless. Imprint is meant to be stateful.

That means it is built around continuity:

- who the student is
- what they care about
- what is already pending
- what has already been started
- what should happen next

It is also intentionally narrow. This is not trying to be a general-purpose desktop agent for everything. It is focused on one specific domain where continuity actually matters: student workflow.

## Product Scope

The current version focuses on:

- student profile and local memory
- Gmail, Classroom, and Calendar context
- academic prioritization
- browser-based execution
- document creation for work in progress

It intentionally does not try to be:

- a generic productivity suite
- a broad operating system agent
- a note-taking app
- a visual graph analytics product

The goal is a small, useful product with a clear point of view.

## Submission Story

The idea behind Imprint is simple:

> students do not need another stateless assistant.  
> they need an agent that remembers context, understands priorities, and helps them move real academic tasks forward.

That is what this project explores.

## Demo Flow

The product is best demonstrated through a concrete student workflow:

1. connect academic tools
2. ask what is pending or urgent
3. let Imprint identify the most relevant next task
4. open the right browser or document context
5. help complete the task

Examples include:

- checking whether an assignment is pending
- opening the right Classroom task
- preparing a draft document
- helping with a form-based workflow

## Demo Video

[![Watch the demo](https://img.youtube.com/vi/QoXDiaIr9b4/maxresdefault.jpg)](https://youtu.be/QoXDiaIr9b4)

## Local-First

Imprint stores the student's profile and working context locally so the experience feels personal and inspectable rather than opaque.

That local memory is important to the product. It is what turns the system from a one-off assistant into a stateful one.

## Download

I built and packaged the downloadable installer on Windows, because that is the machine I had available while building this project.

For Windows users:

- download the installer from the GitHub release
- install the app normally
- add your own local API keys before using it

For macOS users:

- the codebase supports macOS behavior and shortcuts
- but the downloadable macOS app needs to be packaged on a Mac

If you are on a Mac and want to build it yourself:

1. clone this repository
2. install Node.js, Rust, and Xcode Command Line Tools
3. create a local `src-tauri/.env`
4. add your own API keys there
5. run:

```bash
npm install
npm run tauri build
```

That will generate the macOS app bundle from the same codebase.

## Local Setup

Imprint is local-first, so users should provide their own API keys locally on their machine.

Create a file at:

```text
src-tauri/.env
```

Add the required values:

```env
GEMINI_API_KEY=your_key_here
GOOGLE_CLIENT_ID=your_google_oauth_client_id
GOOGLE_CLIENT_SECRET=your_google_oauth_client_secret
```

Why this is done locally:

- each user should control their own keys
- personal Google access should be authorized by the user directly
- the app should not ship with hardcoded credentials

After adding the `.env` values:

1. launch the app
2. sign in with Google
3. grant access to Gmail, Classroom, Calendar, Docs, and Drive when prompted
4. start using the agent

## In One Sentence

**Imprint is a local student agent that understands academic context across email, assignments, deadlines, and browser tasks, then helps the student take the next step.**
