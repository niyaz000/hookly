# Why Hookly

## Where this came from

The creator of this project spent years as a backend engineer at Freshworks, a CRM company where customer interactions drive everything. Part of that work involved building and maintaining scheduling and webhook infrastructure — the kind of systems that are invisible when they work, and catastrophically visible when they don't.

Some examples of what that infrastructure had to handle: a daily cron job that automatically assigns incoming tickets and chat conversations to support agents based on availability and workload; shift management rules that re-route work when agents clock in and out; SLA countdown timers that start the moment a support ticket is created and trigger escalation workflows if they breach; and marketplace integrations where hundreds of third-party apps subscribe to events — ticket status changes, contact updates, deal transitions — via webhooks that have to be delivered reliably, in order, at scale.

The platform that handled all of this suffered from compounding problems:

- **Inconsistency** — timers would silently skip or fire twice depending on which worker picked them up
- **Outages** — a single misconfigured tenant or burst of traffic could starve the entire queue
- **Opacity** — there was no easy answer to "why didn't this webhook fire?" Reconstructing what happened required digging through logs across multiple services
- **Scaling friction** — going from thousands to hundreds of thousands of events per minute required significant re-architecture, not just more instances
- **Oncall burden** — not dramatic incidents, but a steady background noise of things that needed a human to unstick them

Those problems are not unique to one company. They are the default state of scheduling and delivery infrastructure when it grows faster than it is designed.

---

## The thought experiment

Hookly is a question: **if you started from scratch today, knowing what you know, how would you build this?**

Not with unlimited budget — with the opposite constraint. With the discipline of a team that could go broke tomorrow if they over-engineered it. With the hardness of a system that has to survive an on-call rotation of one.

And with one new variable: **we have AI now**. Not AI as a silver bullet — AI as a force multiplier. A system designed from the start to be AI-augmented rather than retrofitted.

The result is an infrastructure layer for event-driven systems that takes no shortcuts on correctness, charges minimal operational overhead, and treats developer experience as a first-class feature — not a polish step.

---

## Vision

Hookly aspires to be the infrastructure layer that any team reaches for when they need reliable, observable, and secure event delivery — without needing a platform engineering team to run it.

A solo developer bootstrapping a SaaS product and a Freshworks-scale enterprise managing millions of customer interactions per day should be able to run Hookly without changing the application code. The difference is configuration, not architecture.

Hookly aspires to be the tool where:

- A developer who has never touched the system can understand exactly what happened to any event in under five minutes
- A two-person team can operate it at enterprise scale without burning out on oncall
- Tenants trust that their data, their secrets, and their delivery guarantees are never at risk from another tenant's behavior
- The answer to "why didn't this fire?" is always self-serve — no support ticket, no log spelunking, no tribal knowledge required

It is not a goal to out-feature existing webhook platforms. It is a goal to be the platform you trust completely — because it is simple enough to understand, honest enough to tell you what it is doing, and disciplined enough to never surprise you.

---

## North star

> **Any developer should be able to understand exactly what happened to any event, at any time, with zero tribal knowledge.**

Everything else flows from that:

- If you can explain what happened, you can reproduce it
- If you can reproduce it, you can fix it
- If you can fix it without help, oncall burden drops
- If oncall burden drops, the system is trustworthy
- If the system is trustworthy, developers build on top of it without fear

The enemy is opacity. Hidden state. "It works, we just don't know why." Systems that are only navigable by people who have been running them for years.

---

## Guiding principles

### 1. Frugality as a design constraint

Design as if compute costs money. Design as if engineers are expensive. If two approaches solve the same problem equally well, the one with lower operational cost wins — even if the other is technically more elegant.

This means aggressively reducing overhead at every layer: memory, storage, network, log volume, background work. Every resource consumed has a cost; every cost that compounds silently eventually becomes a problem. Frugality is not minimalism for its own sake — it is the discipline that forces good trade-offs and builds systems that can be run cheaply by small teams for a long time.

### 2. Reliability through simplicity

Reliability is not achieved by adding redundancy to a complex system. It is achieved by making the system simple enough that there are fewer things to go wrong.

The fewer moving parts, the fewer failure modes. The fewer failure modes, the fewer pages. The fewer pages, the smaller the team you need to keep things running. Simplicity is a multiplier on reliability, not a trade-off against it.

### 3. Customer obsession: the customer must never feel the infrastructure

No matter what happens inside the system — a Redis restart, a database failover, a scheduler shard crash, a slow external endpoint, a bad deployment — the customer's experience must be preserved. Retries are automatic. Backpressure is absorbed internally. Circuit breakers prevent a failing endpoint from degrading the worker pool. The outbox pattern ensures delivery jobs survive queue restarts.

Every failure mode is designed around one question: **does the customer ever notice?**

The system must absorb failures, not surface them. An infrastructure incident that requires no human intervention and leaves no customer-visible scar is the goal. If a customer has to open a support ticket because an internal component misbehaved, that is a design failure — not an operational one. The infrastructure layer exists precisely to ensure that "crazy things happening inside" never translates into degraded experience outside.

### 4. Two-person operations ceiling

The entire platform — from deployment to incident response to onboarding a new tenant — should be runnable by a maximum of two people. Any design decision that makes this harder is the wrong decision.

This is the forcing function that prevents accidental complexity. If a new engineer cannot be productive on day one, if an incident cannot be diagnosed without tribal knowledge, if scaling requires coordination across multiple teams — those are design failures, not operational realities.

### 5. Performance as a first-class concern

Low latency and high throughput are not features to be added later — they are properties the system must carry from the first line of code. Sub-second API responses, efficient queue processing, and minimal overhead per delivery attempt are non-negotiable.

Performance work that cannot be done frugally — through better algorithms, smarter batching, reduced allocations — is preferred over performance work that requires more hardware. Spend CPU cycles wisely before spending money.

### 6. Observability for everyone

Debugging should not require access to production databases or internal tooling. Tenants should be able to answer "why didn't my webhook fire?" from a self-serve interface. Operators should be able to see the health of the entire platform at a glance.

Observability is a product feature, not an internal tool. If a tenant has to open a support ticket to understand what happened to their event, the system has failed them. Every delivery attempt, every failure, every retry is a fact that should be surfaced — to the right person, at the right level of detail.

### 7. Security as a first-class citizen

Security is not a layer added at the end. It is a property of the system that every design decision either upholds or erodes.

Tenant data is isolated by default, not by configuration. Credentials are encrypted at rest, always. Every action is attributable to a principal. Every sensitive operation leaves an audit trail. There are no back doors, no "internal" paths that bypass the permission model.

### 8. Access control for everything

Every resource has an owner. Every operation is authorized. Every credential has a scope. There is no concept of privileged internal access that bypasses the permission system — the same rules apply to operators, tenants, and automated systems.

Fine-grained access control is not a feature for enterprise customers. It is the default from day one.

### 9. Tenant isolation: one bad actor cannot wreck others

In a multi-tenant system, the most important reliability guarantee is that one tenant's behavior cannot degrade another's experience. A misconfigured endpoint, a burst of events, a slow consumer — none of these should propagate beyond the tenant boundary.

Fair usage limits and per-tenant resource controls are not nice-to-haves. They are the enforcement mechanism for a core promise.

### 10. Developer experience is a first-class feature

The difference between a tool developers love and one they tolerate is almost never the core feature set. It is the hundred small things: clear error messages, consistent response shapes, predictable behavior, IDs that tell you what resource they belong to.

The path from "I want to deliver webhooks" to "webhooks are delivering" should be as short as possible. Sane defaults remove decision fatigue for the common case. Good developer experience is not polish — it is a force multiplier on adoption, debugging speed, and operational confidence.

### 11. Battle-tested components only

No alpha-stage dependencies. No novel consensus protocols. No databases we cannot hire engineers for. No infrastructure that requires specialist knowledge to operate.

When something goes wrong at 3am, the engineer on call should be able to find a well-understood runbook — not debug an obscure distributed systems primitive. The boring choice is usually the right choice.

### 12. Minimal external dependencies

Every external dependency is a failure mode, a scaling bottleneck, and a cost center. Dependencies should be chosen deliberately, kept to the minimum necessary, and replaced with simpler alternatives wherever possible.

Vendor lock-in at the infrastructure layer is not a constraint we accept. The system should be deployable on-premises or across any cloud provider without changing application code.

### 13. Auditing as a core feature

Every configuration change, every credential rotation, every delivery attempt is a fact that should be recorded and queryable. Audit trails are not compliance theatre — they are the foundation of debuggability, security forensics, and customer trust.

An event-driven system that cannot explain its own history is not trustworthy.

### 14. Automation and self-healing

Manual intervention is a cost. Every time a human has to restart a process, retry a delivery, or clear a stuck queue, that is time and attention that should have been spent on something else.

The system should detect and recover from common failure modes automatically. Where full automation is not possible, it should surface the problem clearly and provide the tooling to resolve it quickly — without requiring a specialist.

### 15. AI-native, not AI-bolted-on

This system was designed alongside AI tooling, not before it. Code is written to be readable by a language model and a human equally. Error messages are structured to be parseable, not just human-readable. Every architectural decision is documented so that an AI assistant can reconstruct context without access to Slack history or tribal memory.

The future includes AI-driven anomaly detection, smart retry policies, auto-generated runbooks, and an onboarding assistant that answers "how do I subscribe to this event?" from the live catalog. These are not retrofits — they are the reason the foundation is built the way it is.
