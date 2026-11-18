# Customer scenario workspace

Owns the private customer cohort editor, native run progress and saved comparisons.
Consumes Studio scope and the registered scenario/task API; it owns no commerce,
identity, inference or publication logic. Reports contain fictional profiles and
native observations, never provider credentials or cart capabilities.

`ScenarioStudio` coordinates scoped requests; `ScenarioSuggestions` previews
bounded native catalogue-derived fictional profiles before explicit adoption,
without customer records, inference, auto-start or a second executor; `ScenarioReport` renders evidence;
`scenario-types` defines safe projections; `scenario-i18n` supplies EN/DE/FR/ES;
`scenarios.css` owns responsive presentation. Browser coverage and conversion
predictions must not be inferred from these API journeys.

`ScenarioAttacks` runs the scripted attack test (`POST /api/agent/scenario-probes`)
for the selected sandbox and lists native refusals; `scenario-attack-i18n` holds
its closed probe and finding vocabulary. Results are not saved and are not a
security certification of the shop.

`ScenarioTwin` reads `GET /api/agent/scenario-readiness` for the selected
sandbox and lists catalog, test stock, simulated checkout and saved tests with a
link to the owning workspace for each open step; `scenario-twin-i18n` holds its
vocabulary. It writes nothing.
