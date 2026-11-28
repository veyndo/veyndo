# Proposal twin review

`ProposalSimulation.tsx` renders the shop twin result and the explanation that the
server stores with every agent proposal (`simulation` and `explanation` in the
task evidence). It shows the field diff, the projected impact with its assumptions
and the rule findings side by side, names proposal parts the twin cannot simulate,
and lists each agent action with its rationale and source references. Model text
is shown as unverified text, never as markup.

`proposal-simulation-model.ts` holds the wire types and the pure decisions:
`approvable` (no approval after a failed replay or a blocking violation; the server
refuses the same), rule ordering, source resolution and money formatting.
`proposal-simulation-i18n.ts` has one entry per stable server code in English,
German, French and Spanish; an unknown future code is shown as the code itself.

The shell's `ProposalCard` mounts this review. The data comes from the existing
run status and `merchant.run.explain`; this folder adds no API and no state. See
[the shop twin guide](../../../../../docs/shop-twin.md#simulate-agent-proposals-before-you-approve-them).
