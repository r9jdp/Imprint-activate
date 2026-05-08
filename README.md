# Activate AI Fellows Submission

## Project

**Imprint** is a local, stateful browser agent for students.

It connects a student's academic world across Gmail, Google Classroom, Google Calendar, browser state, and local memory. Instead of acting like a stateless assistant that only understands the current page, Imprint builds a persistent context graph about the student, their priorities, their pending work, and their recent high-signal communications.

The goal is not to build a generic desktop copilot. The goal is to build the smallest useful version of a student agent that can:

- understand what the student should focus on
- remember why it matters
- open the right browser context
- create working artifacts like drafts and documents
- keep all of that state locally on the student's device

## Core Idea

Students live across fragmented systems:

- assignment instructions are in Google Classroom
- deadline changes arrive through email
- due dates live in Calendar
- important goals like placements and internships are never reflected in those tools

As a result, students are forced to manually stitch everything together every day.

Imprint turns that fragmented workflow into a **stateful execution loop**:

1. understand the student through a short onboarding flow
2. build a local profile and context graph
3. continuously interpret recent emails, assignments, deadlines, and current browser pages through that graph
4. tell the student what matters most right now
5. take browser actions to help them actually complete the work

## Product Thesis

> **A local student agent should not just answer questions about the current page. It should understand who the student is, what they care about, what is pending, and what action should be taken next.**

## Why This Exists

Most assistants are stateless.

They can:

- summarize the current page
- answer a one-off question
- help with a single task

But they usually do not know:

- whether the student is prioritizing placements or grades
- which emails are personally important
- what deadlines are already pending
- whether the current page relates to an unfinished task
- what document or draft has already been created for that assignment

Imprint exists to make the browser feel like a stateful academic workspace instead of a stream of disconnected tabs.

## What Makes It Interesting

### 1. Student-specific, not generic

This is not a general-purpose browser agent.

It is designed for one concrete persona:

- a student juggling assignments, deadlines, placement opportunities, and academic communication

That lets the product make better decisions than a generic assistant.

### 2. Local student profile and onboarding

At first launch, Imprint asks a small set of onboarding questions and generates local memory such as `user.md`.

This profile captures:

- academic identity
- current priorities
- important email categories
- goals like placements, internships, assignments, or exams
- output preferences
- reminder preferences

This gives the system stable context before it starts acting.

### 3. Context graph instead of flat memory

Imprint maintains a local context graph with relationships across:

- student profile
- courses
- assignments
- deadlines
- emails
- reminders
- resources
- drafts
- browser tasks

This makes it possible to reason about:

- what is urgent
- what is high-value for this particular student
- what the student is ignoring
- what should be done first

### 4. Recent email and pending task prioritization

The system does not just surface unread emails.

It should be able to say:

- this placement cell email matters more than the club newsletter
- this assignment matters less than an internship deadline tomorrow
- this professor email changes how a pending submission should be handled

That is where the local graph becomes practically useful.

### 5. Execution, not just organization

Imprint is not only a reminder system.

Using browser control, it can:

- open the top-priority assignment
- read instructions
- open related emails or resources
- create a new Google Doc
- write a first draft
- prepare the browser context for the next step

The product is meant to help students act, not just observe.

## Smallest Useful Version

The intentionally scoped version of Imprint includes:

- a global overlay or summonable browser control surface
- a short onboarding interview
- local student memory generation (`user.md`)
- a local context graph for the student and recent activity
- reading Google Classroom, Gmail, and Calendar context
- ranking what the student should focus on
- opening the most relevant browser pages
- creating a first draft document for a selected assignment

## Current Build Status

The current implementation now includes:

- summonable desktop shell UI
- Google OAuth sign-in from local env configuration
- live reads from Gmail, Google Classroom, and Google Calendar
- local student profile onboarding stored on-device
- prompt-driven Gemini agent that can use browser tools for web tasks
- a Workspace snapshot dashboard for recent email, coursework, and events
- a Gemini-powered query box that uses student profile + Workspace context
- browser control integrated into the agent flow for task execution

The current Gemini assistant call uses:

- `gemini-3-flash-preview`

Still intentionally pending:

- local context graph
- reminders and personalized ranking logic
- Google Docs / Drive integration
- polished submission website with demo video and download flow

The repository will also include a separate submission website in a dedicated `frontend/` folder.
That site is intended to:

- explain the product clearly
- match the desktop agent visual theme
- embed the demo video
- provide a download/install entry point for the agent

What it intentionally leaves out:

- generic desktop automation
- plugin ecosystems
- broad multi-domain agent behavior
- full visual graph analytics product
- trying to automate every part of student life

The aim is to keep the product focused, demoable, and obviously useful.

## Primary Use Case

### Stateful academic execution

A student asks:

> “What should I focus on right now?”

Imprint checks:

- pending Classroom assignments
- recent important emails
- upcoming Calendar deadlines
- the student's profile and priorities

It then answers:

- what is pending
- what is personally most important
- what should be done first

Then the student asks:

> “Handle the top priority.”

Imprint can:

- open the relevant assignment
- read its instructions
- open the related email/resource
- summarize the work
- create a draft document
- update local state so it remembers that work has started

## Example Demo Scenario

A student has:

- multiple pending assignments
- one placement cell email with a deadline tomorrow
- a professor update that changes submission instructions

The student opens Imprint and asks:

1. “What should I focus on right now?”
2. “Open the most urgent item and gather everything I need.”
3. “Create a document and write a first draft for this assignment.”
4. “Save this as in progress and remind me before the deadline.”

This flow demonstrates:

- statefulness
- prioritization
- local memory
- context graph reasoning
- browser execution

## Graph Design

The graph has two layers.

### Stable profile graph

Captures who the student is:

- degree
- branch
- semester
- goals
- priorities
- interests
- output preferences
- important communication categories

### Live activity graph

Captures what is changing:

- recent emails
- assignments
- deadlines
- reminders
- resources
- created drafts
- submission state

These layers let the agent move from generic urgency to **personalized urgency**.

## Local-First Memory

Imprint stores its memory locally.

That includes:

- `user.md` for readable profile context
- structured graph data for machine reasoning
- optional recent activity state

This is important because the product is meant to feel personal and trustworthy. The student should be able to inspect, edit, and control the memory directly.

## Why This Is Different

This project is different from a generic browser assistant because it is:

- **stateful**, not stateless
- **student-specific**, not broad and vague
- **local-first**, not remote-memory-first
- **executional**, not only informational
- **priority-aware**, not merely reactive

The key novelty is not “AI can browse.”

The key novelty is:

> **the browser agent maintains an evolving local model of the student and their academic context, then uses that state to decide what matters and what to do next.**

## Design Judgment

The larger underlying prototype included broader agent capabilities. For this submission, the right move is to narrow the product around a sharper use case:

- student workflow
- local memory
- task prioritization
- browser execution

That keeps the scope small enough to ship while still making the product feel original and useful.

## What Success Looks Like

A strong version of this product should make a reviewer feel:

- “This solves a real student problem.”
- “The stateful memory is doing meaningful work.”
- “The agent is not just summarizing; it is helping execute.”
- “This feels focused rather than overbuilt.”

## In One Sentence

**Imprint is a local, stateful student browser agent that builds a personal context graph from profile, email, assignments, and deadlines, then uses that state to prioritize and execute academic tasks.**
