# Connect OpenAI, Claude and other chat clients

There are two different connections, and they are easy to confuse.

| You want | Connection | Section |
| --- | --- | --- |
| The assistant **inside Studio** to use a model | A model provider configured on the server | [Models inside the merchant chat](#models-inside-the-merchant-chat) |
| Claude Desktop or another **local** client to use your shop | The stdio bridge to your shop's MCP endpoint | [Claude Desktop / local MCP clients](#claude-desktop--local-mcp-clients) |
| ChatGPT or Claude **in the cloud** to use your shop | The shop's HTTPS `/mcp` endpoint | [ChatGPT / Claude remote connectors](#chatgpt--claude-remote-connectors) |

Both directions use the same commerce operations and the same permission
checks. To see the tool calls an agent makes before you connect one, follow
[Let an agent buy](agent-purchase.md).

## Models inside the merchant chat

1. **Configure a provider.** Operators set encrypted global provider settings
   and shop overrides in **VEYNDO Platform → AI providers**. Ordinary shops
   inherit that selection. See
   [provider inheritance](platform.md#central-ai-inherited-by-all-shops).
   For a local setup, use the `.env` entries below.
2. **Choose it in Studio.** In `/#merchant`, sign in with a personal member
   account and open the assistant model settings. Choose Local, OpenAI or
   Claude. You can override the model ID per turn.
3. **Ask for a bounded change** and inspect the stored fields before you
   approve.

Local setup: put the credentials into the private, ignored `.env` and restart
the server.

```dotenv
OPENAI_API_KEY=your-api-key
OPENAI_MODEL=gpt-6-sol
ANTHROPIC_API_KEY=your-api-key
ANTHROPIC_MODEL=claude-sonnet-5-5
```

What to know before you choose a cloud provider:

- These APIs need their own provider credentials and may incur charges. No
  subscription cookies, ChatGPT browser automation or Claude subscription
  tokens are used. The database and the local-model route need no paid service.
- Cloud selection sends the bounded catalog, graph context and conversation
  history to that provider.
- A configured flag is not a health check of the provider.
- Credentials are never returned to the frontend or stored in conversations.
- There is no silent fallback to another provider. A failure is stored as an
  explicit error message and never creates an executable proposal.

How the server uses the model: OpenAI **Responses** with structured outputs,
or Anthropic **Messages** with `output_config.format`. The server validates
IDs, ranges and trusted revisions for every provider. Merchant planning uses
bounded native provider tool-call rounds for authorized reads before producing
the final structured proposal: OpenAI Responses/Chat, Claude Messages and
Ollama share the native operation authorization. A tool result does not grant
additional permissions or approve a write.

Official references: [OpenAI structured outputs](https://developers.openai.com/api/docs/guides/structured-outputs),
[Claude structured outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs).

## Claude Desktop / local MCP clients

A small bridge forwards JSON-RPC requests over stdio to your running shop. It
is transport only and contains no commerce logic.

1. Start your shop ([first tour](first-tour.md) or [quickstart](quickstart.md)).
2. Add this entry to your client's MCP configuration and replace the absolute
   checkout path. Keep the entries that are already there.
3. Restart the client. It now lists the customer and read tools.

```json
{
  "mcpServers": {
    "veyndo": {
      "command": "python3",
      "args": ["/absolute/path/veyndo/scripts/mcp_stdio.py"],
      "env": {
        "COMMERCE_URL": "http://127.0.0.1:8787",
        "COMMERCE_TENANT": "atelier"
      }
    }
  }
}
```

This configuration exposes the customer and read tools. Merchant work needs a
credential:

| Goal | Credential | What it can do |
| --- | --- | --- |
| Let an agent plan merchant changes | A **Dedicated agent** access from **Developer → API & integrations**, with [explicit model, token and USD allowances and read scopes](agent-authority-controls.md) | Prepares a preview. It cannot perform the final apply. |
| Work interactively as yourself | Your personal session, see [credentials](#personal-and-delegated-workspace-credentials) | `merchant.plan` → inspect the stored preview → `merchant.apply` with explicit approval, within your rights. |

Set `COMMERCE_SESSION_TOKEN` privately to the credential and `COMMERCE_TENANT`
to its shop. Do not commit a filled configuration. Ordinary integration keys
cannot start unbudgeted native AI planning. The merchant reviews and approves
business changes through their personal Studio session. `knowledge.graph` and
`knowledge.search` expose the actual graph and retrieval route.

The allowance covers calls to Veyndo's central model boundary. A separate model
run by an external MCP client has its own provider billing; these counters do not
measure or cap that client's independent token use.

## ChatGPT / Claude remote connectors

Hosted ChatGPT and Claude cannot reach localhost. You need a deployed HTTPS
endpoint or a supported secure tunnel. Keep the local demo bound to loopback
until its production identity layer exists.

The endpoint is `/mcp`, a stateless JSON-response subset of Streamable HTTP.
GET returns 405 because this prototype offers no SSE subscription.

For an authorized OpenAI API client, the remote MCP tool configuration is:

```json
{
  "type": "mcp",
  "server_label": "commerce",
  "server_url": "https://your-commerce-host.example/mcp",
  "authorization": "Bearer YOUR_PRIVATE_MERCHANT_TOKEN",
  "require_approval": "always"
}
```

For ChatGPT's UI, an authorized account/workspace owner must add the custom
MCP app in Developer Mode. Account availability and authentication requirements
depend on the product/plan. This repository does not register an app in your
ChatGPT/Claude account or implement a full OAuth authorization server.
For a remote UI connector that requires OAuth, an OAuth gateway is still needed;
a personal session or integration key is not a replacement for that OAuth gateway.

[ChatGPT developer-mode setup and secure-tunnel guidance](https://help.openai.com/en/articles/12584461-developer-mode-and-full-mcp-connectors-in-chatgpt)
and [OpenAI remote MCP API](https://developers.openai.com/api/docs/guides/tools-connectors-mcp).

## Verification boundary

`scripts/providers.py` exercises both adapters against **local HTTP contract
servers** and verifies the real merchant conversation and approval routes.
Those tests do not prove live OpenAI/Anthropic inference. Real local Ollama
inference is tested separately. Live cloud checks require your own API keys;
no borrowed credentials or fabricated cloud results are included.

## Personal and delegated workspace credentials

For interactive personal merchant MCP access, set `COMMERCE_SESSION_TOKEN` to that user's
opaque login session and `COMMERCE_TENANT` to one of their memberships. The bridge
uses this token preferentially. Knowledge and merchant tools check scoped
authority on the server; readers cannot apply proposals. Sessions expire after
12 hours and must be renewed through personal login. Never distribute the
legacy global `MERCHANT_TOKEN` to SaaS merchants. Public product/cart tools work
without merchant credentials; complete graph/vector tools do not. The generated
local configuration intentionally contains no embedded credentials.

Delegated native AI planning instead uses a dedicated agent key with its own
configured expiry and model allowances. Browser Studio sign-in uses HttpOnly
cookies; do not extract or distribute browser cookies as MCP credentials.
[Browser-session boundary](browser-sessions.md).
