/** The rendered component catalogue: every kit/ui part with its states, side by side in the light and dark theme. */
import { useState, type ReactNode } from "react";
import {
  Badge,
  Button,
  Card,
  Checkbox,
  Dialog,
  EmptyState,
  Field,
  Menu,
  Notice,
  Select,
  Spinner,
  Stack,
  Switch,
  Tabs,
  TextArea,
  TextInput,
  catalogue,
} from "../../../../../../frontend/src/kit/ui";
import { useCatalogueText } from "./catalogue-i18n";
import "./ui-catalogue.css";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="ui-catalogue-section">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function Specimens() {
  const t = useCatalogueText();
  const [dialog, setDialog] = useState(false);
  return (
    <Stack gap="6">
      <Section title={t("buttons")}>
        <Stack direction="row" wrap gap="2" align="center">
          <Button tone="primary">{t("primary")}</Button>
          <Button>{t("secondary")}</Button>
          <Button tone="ghost">{t("ghost")}</Button>
          <Button tone="danger">{t("danger")}</Button>
          <Button tone="primary" busy>
            {t("busy")}
          </Button>
          <Button size="sm">{t("secondary")}</Button>
          <Button disabled>{t("primary")}</Button>
        </Stack>
      </Section>
      <Section title={t("status")}>
        <Stack gap="3">
          <Stack direction="row" wrap gap="2" align="center">
            <Badge>{t("neutral")}</Badge>
            <Badge tone="accent">{t("accent")}</Badge>
            <Badge tone="positive">{t("positive")}</Badge>
            <Badge tone="warning">{t("warning")}</Badge>
            <Badge tone="danger">{t("failed")}</Badge>
            <Spinner label={t("loading")} />
          </Stack>
          <Notice tone="positive">{t("notice")}</Notice>
          <Notice
            tone="danger"
            title={t("failed")}
            action={<Button size="sm">{t("retry")}</Button>}
          >
            {t("alert")}
          </Notice>
        </Stack>
      </Section>
      <Section title={t("forms")}>
        <Stack gap="4">
          <Field label={t("name")} hint={t("nameHint")} required>
            <TextInput />
          </Field>
          <Field label={t("email")} error={t("emailError")}>
            <TextInput type="email" />
          </Field>
          <Field label={t("country")}>
            <Select defaultValue="DE">
              <option value="DE">{t("germany")}</option>
              <option value="ES">{t("spain")}</option>
            </Select>
          </Field>
          <Field label={t("note")}>
            <TextArea rows={2} />
          </Field>
          <Checkbox label={t("terms")} defaultChecked />
          <Switch label={t("active")} defaultChecked />
        </Stack>
      </Section>
      <Section title={t("navigation")}>
        <Stack gap="4">
          <Tabs
            label={t("navigation")}
            items={[
              { id: "overview", label: t("overview"), panel: t("notice") },
              { id: "details", label: t("details"), panel: t("nameHint") },
              {
                id: "history",
                label: t("history"),
                panel: t("emptyBody"),
                disabled: true,
              },
            ]}
          />
          <Menu
            label={t("actions")}
            items={[
              { id: "duplicate", label: t("duplicate"), onSelect: () => {} },
              { id: "archive", label: t("archive"), onSelect: () => {} },
              {
                id: "delete",
                label: t("danger"),
                onSelect: () => {},
                tone: "danger",
              },
            ]}
          />
        </Stack>
      </Section>
      <Section title={t("overlays")}>
        <Stack gap="4">
          <Button onClick={() => setDialog(true)}>{t("openDialog")}</Button>
          {dialog && (
            <Dialog
              title={t("dialogTitle")}
              description={t("dialogBody")}
              onClose={() => setDialog(false)}
              footer={
                <>
                  <Button onClick={() => setDialog(false)}>
                    {t("secondary")}
                  </Button>
                  <Button tone="danger" onClick={() => setDialog(false)}>
                    {t("danger")}
                  </Button>
                </>
              }
            />
          )}
          <Card
            title={t("card")}
            actions={<Badge tone="accent">{t("accent")}</Badge>}
          >
            <EmptyState title={t("empty")}>{t("emptyBody")}</EmptyState>
          </Card>
        </Stack>
      </Section>
    </Stack>
  );
}

export default function UICatalogue() {
  const t = useCatalogueText();
  return (
    <div className="studio-page ui-catalogue">
      <div className="page-intro">
        <h1>{t("title")}</h1>
        <p>{t("intro")}</p>
      </div>
      <div className="ui-catalogue-themes">
        {(["light", "dark"] as const).map((theme) => (
          <div key={theme} data-theme={theme} className="ui-catalogue-theme">
            <h2>{t(theme)}</h2>
            <Specimens />
          </div>
        ))}
      </div>
      <Section title={t("index")}>
        <ul className="ui-catalogue-index">
          {catalogue.map((entry) => (
            <li key={entry.name}>
              <code>{entry.name}</code>
              <Badge tone={entry.kind === "molecule" ? "accent" : "neutral"}>
                {t(entry.kind)}
              </Badge>
              {entry.pattern && <code>{entry.pattern}</code>}
            </li>
          ))}
        </ul>
      </Section>
    </div>
  );
}
