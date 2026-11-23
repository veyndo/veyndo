# src/cognition ownership

Root AGENTS.md also applies. Rules below were moved from the root, not weakened.

## Cognitive commerce continuity

Merchant planning continuation belongs to existing `tasks`, conversations and
`components/intelligence/src/service/{agent_runs,run_journal,planner}.rs` (the
server executes runs over the agent port in `src/agent_host.rs`); read `docs/durable-agent-runs.md`. Keep native read tools, central
inference and original proposal approval. Store private completed checkpoints,
recheck credential/scope/evidence, fence completion by its lease and explicitly
continue uncertain calls. Detect request-ID replays before new attempt quota
admission. Never store opaque credentials, auto-repeat ambiguous calls, add a
second queue or turn resume into automatic writes. Keep actual worker-kill,
HTTP restart, revocation, source-withdrawal and duplicate-request regressions.

Read [the connected cognition guide](../../docs/cognitive-commerce.md) before changing
intelligence. It records exact implemented paths and the still-open audit scope.
`components/intelligence` owns source review, read-agent rounds
(`service/read_rounds`), price guardrails and autonomy, controlled experiments,
consent-bound preferences, retrieval and the rebuildable Qdrant projection
(`knowledge`) and the consent receipts that gate personalization and analytics;
`src/cognition/` only registers routes. Extend the existing settings, capability, outbox,
chat, proposal and native checkout owners. Do not create another knowledge truth
store, pricing engine, approval queue or chat backend. Stored evidence is not
model-weight learning; exact source admission is not proof of surrounding prose.
Use one consistent versioned embedding model across replicas. Automatic model
cursors, source-trigger jobs and fenced publication share the existing index queues.

## Behavioral intelligence ownership

Consent-bound native visitor capture (`components/intelligence/src/http/behavior_capture.rs`),
the consent receipts (`service/consent.rs`) and the report and journey reads
(`service/behavior.rs`) live in the intelligence component. Reuse `session_signal_events`, native carts/accounts,
current legal receipts, product knowledge, forced tenant RLS and composite foreign
keys. Never introduce another analytics identity, cookie, event ledger, pricing or
order truth. Do not record raw IP/UA, search/question text or visitor-supplied customer
IDs. Reports need `knowledge.read`; individual journeys need `customers.pii` and
entity links require their existing read rights. Geography requires a trusted
server ingress; client country headers are not authoritative. Native storefront
capture and Studio use the same HTTP/MCP contract. External frontends must adopt
the consent-bound API explicitly; a configured Storyfront does not imply capture.
Run `behavior_intelligence`, privacy/isolation suites and behavioral UI regressions.
See `docs/behavior-intelligence.md` for retention, counts and learning boundaries.

