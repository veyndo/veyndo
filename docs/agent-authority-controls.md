# Delegate an agent, keep control of the shop

An agent receives its own revocable credential, a small set of read permissions
and an explicit model allowance. It can inspect authorized information and prepare
a proposal. A person still approves business changes through the native operation.

**Implementation state:** deployed and publicly exercised, 16 October 2026.
[Hosted receipt and boundaries](evidence/agent-authority-hosted-2026-10-16.md) record the
actual stop, purchase and session journeys. [Current public status](status.md) remains
the release authority. These controls do not make the whole app-concept programme complete.

## Configure a planning agent

Select the **Live shop**, then open **Developer → API & integrations** with a personal account that can manage
team access. Agent credentials are live operational controls; creating one from a sandbox is rejected before issuance. Choose **Dedicated agent**, give it a recognizable name and select
only the data it needs. The scope is the intersection of the key's grants and its
creator's current membership; removing a role or revoking the key takes effect
on subsequent admission and saved-run continuation.

Specify all of the following; there are no unlimited defaults:

| Field | Meaning |
| --- | --- |
| Provider and exact model | OpenAI, Anthropic or Ollama and the exact configured model name. An omitted request model selects this approved pair; an explicit conflicting pair is denied. |
| Lifetime token allowance | Total conservative request allocations, across replicas, process restarts and sandboxes. |
| Allocation budget in USD | Total allocation at the approved ceiling tariff, stored as integer micro-USD. |
| Ceiling price per million tokens | An administrator-selected costing rule. Verify it against the provider/model you configure. |
| Permissions and expiry | Explicit read scopes, optional assistant use and separately granted personal-data access. Existing personal expiry rules apply. |

Copy the credential once and keep it outside prompts, source code and browser
storage. Subsequent lists show identity, permissions and counters, never plaintext.
A revoked agent key retains its accounting row. Replacing it creates a new identity
with a new allowance; agents cannot issue credentials or increase their own limits.

Agent profiles accept read permissions, `assistant.use` and explicitly granted
`customers.pii`. They cannot complete purchases, apply proposals, mutate apps,
create keys or invoke arbitrary app callbacks. HTTP admission and MCP discovery/
invocation enforce this, rather than relying on model instructions.

**Compatibility:** ordinary API keys remain available for their explicit native
integration operations. To start saved merchant-agent work or use central model
planning with a nonpersonal integration credential, create a dedicated bounded
agent key. A generic key is no longer an unmetered native AI credential.

## Check a plan before it runs

An agent or MCP client can read `merchant.operations.list` for the operations it may
call, with permission, cost, reversibility, risk, affected data and side effects, and
send a planned sequence to `merchant.operations.check`. The check names every step that
needs approval (high risk or irreversible), that the credential cannot run, or whose
arguments fail the published input contract. It executes nothing and grants nothing:
approval still happens through the proposal review or the native operation by a person.
See [the typed operations graph](business-capabilities.md#the-typed-operations-graph).

## What is counted

Before each text-model request, PostgreSQL locks the live shop and key, checks
current admission, reserves the complete envelope, records an allocation event and
commits. Live-root locks use `FOR NO KEY UPDATE`: they serialize agent controls and allocations while remaining compatible with commerce foreign-key reference readers. This avoids a tenant/task lock-order inversion at worker completion. Only then may the provider call start. Two Rust replicas cannot spend the
same remaining allocation. No transaction stays open while waiting for the model. The authority scope constructs a boxed child synchronously, preserving a small polling boundary for large native HTTP/POS futures; it does not enlarge worker stacks.

The envelope includes serialized UTF-8 request bytes, the bounded output allowance
and 4,096 units of framing headroom. Its cost is rounded upward from the approved
integer tariff. Successful, failed and uncertain requests retain their allocation;
continuation reusing a completed checkpoint does not allocate that call again.

**These are conservative allocation counters, not actual provider token invoices
or a guaranteed provider billing ceiling.** They do not reconcile credits or refunds.
Budgeted agents currently accept text-only model requests; paid query embeddings
and reranking are skipped in favour of native lexical retrieval. Background AI
jobs, independently operated app services and ordinary human interactive use are
not all covered by this per-key envelope. Existing daily attempt and shared
concurrency limits remain separate controls.

Exhaustion returns HTTP 429 with `kind: quota_exceeded`, `retryable: false` and
`pointer: /agentBudget`; it has no misleading short `Retry-After`. An administrator
must review a replacement delegation rather than repeatedly retrying an exhausted key.
Studio explains the administrator's stop and persistent allowance exhaustion in
all four interface languages. Its localized recovery guidance is visible directly
in the error notice, rather than hidden under a technical-details disclosure.

## Stop the shop's merchant agents

A personal user with `ai.manage` sees **Agent control** in the same API workspace.
Stopping requires an explicit confirmation and current revision. The switch
always targets the live shop **and all its private sandboxes**, even when opened
from a sandbox. It cancels queued, running and uncertain native merchant tasks,
fences late worker results and blocks agent-key access and subsequent scoped
central model admission. An agent or ordinary integration key cannot lift it.

Already admitted or sent provider requests may finish and may be charged; the stop
cannot recall network requests. Late results from cancelled durable runs cannot
become executable proposals. Normal shopping remains available. Enabling the
switch permits new work; cancelled tasks do not restart automatically.

This is not a kill switch for every background indexer or independently operated
external service. The switch, credentials and allocation counters are operational:
configuration history, sandbox cloning and selected publication cannot restore or
increase them. Original financial approvals and domain invariants remain authoritative.

## Simulate, explain, then approve

Every proposal a merchant agent prepares is replayed in the
[shop twin](shop-twin.md#simulate-agent-proposals-before-you-approve-them) before
review. Studio shows the field diff, the projected 30-day impact with its
assumptions and the rules it checked side by side. A failed replay or a blocking
violation (for example a price below the corridor minimum) disables **Approve**,
and the server refuses it as well: the corridor is checked again on the current
shop when you approve, and HTTP, MCP and the price autopilot share that refusal.

A corridor violation no longer stops the agent run. It is saved as a proposal
with a blocking finding, so you see which limit the agent missed instead of a
failed run. It still cannot be applied.

### Explanations

Each agent action records why it happened and what it relied on:

| Action | Rationale | Sources |
| --- | --- | --- |
| Tool call (a read during planning) | Requested by the model while planning | Your instruction (digest) and the read operation with a digest of its input and result |
| Proposal | The model's own `rationale` (or its summary), marked **Unverified model text** | Changed products with their revision, documents with content hash, external sources with digest, installed apps, knowledge graph and shop observations (digests), AI policy revision, the Flow job that started the run, model and prompt with a digest of the output |
| Twin simulation | The simulation status | Each evaluated rule and the recorded-order window (digest) |
| Applied change | Approved after review, or applied within the autopilot policy | AI policy revision, the history entry `agent-proposal:<task>`, the `merchant.change.applied` event and a digest of the simulation |

Sources are references: IDs, revisions and SHA-256 digests. No tool result,
instruction text, credential, prompt or customer record is copied into an
explanation. Model text is untrusted data; Studio renders it as text, never as
markup or instructions. At most 120 sources are stored per run; the rest are
counted.

Every row an applied proposal writes gets the history reason
`agent-proposal:<task>`. The product history panel shows it as
**Agent proposal** with the task ID, so the audit history leads back to the
explanation.

Read an explanation through HTTP or MCP:

| Operation | HTTP | Behavior |
| --- | --- | --- |
| `merchant.run.explain` | `GET /api/agent/runs/{id}/explanation` | Explanation and simulation of a saved run, with the same original-actor, agent-identity and original-scope checks as `merchant.run.status` |

A reader only sees sources its current permissions cover (`catalog.read`,
`knowledge.read`, `apps.read`, `orders.read`, `assistant.use`); the response
counts hidden ones in `hiddenSources`. A merchant's own agent key reads the
explanations of its own runs; it cannot read another agent's or a person's runs,
and another shop gets 404.

## Shared contracts and verification

| Owner | Responsibility |
| --- | --- |
| `integration_keys`, the schema baseline | Existing identity and digest store, exact model policy and durable counters; no new table per agent/app. |
| `src/agent_runs/controls.rs` | Personal revision-bound status/control, planning-only credential admission and per-agent conversation identity. |
| `crates/cognition/src/inference/budget.rs` | Task-local trusted authority and transactional allocations at actual central model egress. |
| Native task/checkpoint/worker owners | Current credential/scope checks and lease-fenced continuation and final persistence. |
| Operation registry | `merchant.agents.status` and `merchant.agents.control`; HTTP `/api/agent/control`, MCP, generated TS/WIT/OpenAPI request contracts. `merchant.run.explain` reads explanations. |
| `components/studio/src/{domain/simulation,service/proposal_simulation}.rs` | Shop twin replay, diff, projection and rule findings stored with the proposal; `service/proposal_apply.rs` refuses failed or blocking simulations and records the applied action. |
| `components/intelligence/src/domain/explanation.rs` | Rationale and source references of every agent action, filtered by current permissions on read. |
| Existing outbox/Flows | `intelligence.agent.allocated` and `intelligence.agents.controlled` carry IDs/counts/state, never credentials or prompt bodies. |

`agent_authorities` runs actual HTTP/PostgreSQL, two Rust replicas and a local
synthetic model provider. It checks allocation races, independent token/cost
exhaustion, exact model mismatch, restart, two agents sharing a human creator,
foreign shops, revocation, sandbox stop, stop under a held commerce-reference lock and denied credential/commerce writes.
The signed-runtime foundation also repeats it through separate restricted tenant
and system logins. `durable_agents` stops a real worker during a provider call and
rejects its late result. UI tests fence late credentials and status from a previous
shop and require explicit limit/confirmation inputs. No real paid provider is used.
`proposal_simulation` runs a price proposal through the twin, approval and apply
with real HTTP/PostgreSQL, an HTTP and a worker process and a synthetic model: the
rolled-back replay, a sale between review and approval, a stale content revision,
a corridor violation, an unsimulatable storefront change, the history reason and
the MCP explanation.

The small arithmetic allowance check reuses the existing extracted kernel guard.
That proof does not certify surrounding SQL, authentication, asynchronous workers
or external providers. [Proof boundary](formal-verification.md).
