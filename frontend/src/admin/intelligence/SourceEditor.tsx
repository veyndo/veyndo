/** Single-language source editor with inherited fields, product lookup and private-first API/file ingestion. */
import { Button, Card, Field, Notice, Select } from "../../kit/ui";
import Icon from "../../shared/ui/Icon";
import { useState } from "react";
import { useLocale } from "../../shared/i18n/i18n";
import { ContentLanguage } from "../../shared/i18n/ContentLanguage";
import ContentLanguagePicker from "../../shared/i18n/ContentLanguagePicker";
import LocalizedField from "../../shared/i18n/LocalizedField";
import type { LocalizedText } from "../../shared/i18n/content-language";
import { useKnowledgeText } from "../../shared/i18n/knowledge-i18n";
import type { RequestFn, Product } from "../shell/studio-types";
import {
  sourceKinds,
  type SourceDetail,
  type SourceKind,
  type Workspace,
} from "./knowledge-types";
export default function SourceEditor({
  source,
  workspace,
  request,
  onSaved,
  onCancel,
  initialKind = "document",
}: {
  source?: SourceDetail;
  initialKind?: SourceKind;
  workspace: Workspace;
  request: RequestFn;
  onSaved: () => void;
  onCancel: () => void;
}) {
  const k = useKnowledgeText(),
    { locale } = useLocale(),
    base =
      source?.locale ??
      (workspace.locales.includes(locale) ? locale : workspace.mainLocale);
  const [language, setLanguage] = useState(base);
  const initial = (field: "title" | "content"): LocalizedText =>
    Object.fromEntries([
      [base, source?.[field] ?? ""],
      ...Object.entries(source?.translations ?? {})
        .filter(([, v]) => v[field] != null)
        .map(([l, v]) => [l, v[field]]),
    ]);
  const [title, setTitle] = useState(initial("title")),
    [content, setContent] = useState(initial("content"));
  const [kind, setKind] = useState<SourceKind>(source?.kind ?? initialKind),
    [productId, setProduct] = useState(source?.product_id ?? "");
  const [query, setQuery] = useState(""),
    [products, setProducts] = useState<Product[]>([]),
    [searching, setSearching] = useState(false);
  const [file, setFile] = useState<File>(),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const translations: SourceDetail["translations"] = {};
      for (const l of workspace.locales.filter((l) => l !== base)) {
        const fields: { title?: string; content?: string } = {};
        if (title[l] != null) fields.title = title[l];
        if (content[l] != null) fields.content = content[l];
        if (Object.keys(fields).length) translations[l] = fields;
      }
      if (file && !source) {
        const form = new FormData();
        form.set("file", file);
        form.set("title", title[base] ?? "");
        form.set("locale", base);
        form.set("kind", kind);
        form.set("translations", JSON.stringify(translations));
        if (productId) form.set("productId", productId);
        await request("/api/knowledge/documents/upload", form);
      } else {
        await request(
          source
            ? `/api/knowledge/documents/${source.id}`
            : "/api/knowledge/documents",
          {
            title: title[base],
            content: content[base],
            locale: base,
            kind,
            productId: productId || null,
            translations,
            ...(source ? { revision: source.revision } : {}),
          },
          source ? "PATCH" : "POST",
        );
      }
      onSaved();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Card className="knowledge-editor">
      <div className="knowledge-section-heading">
        <h2>{source ? source.title : k("newSource")}</h2>
        <Button type="submit" disabled={busy} onClick={onCancel}>
          {k("cancel")}
        </Button>
      </div>
      <ContentLanguage
        locales={workspace.locales}
        mainLocale={workspace.mainLocale}
        language={language}
        onLanguageChange={setLanguage}
      >
        <ContentLanguagePicker />
        <div className="knowledge-form-grid">
          <LocalizedField
            label={k("title")}
            value={title}
            onChange={setTitle}
            maxLength={200}
            externalFallback={
              base !== workspace.mainLocale &&
              title[workspace.mainLocale] == null
                ? (title[base] ?? undefined)
                : undefined
            }
          />
          <Field label={k("kind")}>
            <Select
              value={kind}
              onChange={(e) => setKind(e.target.value as SourceKind)}
            >
              {sourceKinds.map((kind) => (
                <option key={kind} value={kind}>
                  {k(kind)}
                </option>
              ))}
            </Select>
          </Field>
        </div>
        {!file && (
          <LocalizedField
            label={k("content")}
            value={content}
            onChange={setContent}
            multiline
            maxLength={100000}
            externalFallback={
              base !== workspace.mainLocale &&
              content[workspace.mainLocale] == null
                ? (content[base] ?? undefined)
                : undefined
            }
          />
        )}
      </ContentLanguage>
      <div className="knowledge-scope">
        <Field label={k("scope")}>
          <Select
            value={productId}
            onChange={(e) => setProduct(e.target.value)}
          >
            <option value="">{k("shopWide")}</option>
            {productId && !products.some((p) => p.id === productId) && (
              <option value={productId}>{productId}</option>
            )}
            {products.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </Select>
        </Field>
        <form
          className="knowledge-search"
          onSubmit={async (e) => {
            e.preventDefault();
            setSearching(true);
            setError("");
            try {
              setProducts(
                (
                  await request("/api/search/product", {
                    search: query,
                    limit: 30,
                  })
                ).elements,
              );
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setSearching(false);
            }
          }}
        >
          <input
            aria-label={k("productSearch")}
            placeholder={k("productSearch")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <Button type="submit" disabled={searching}>
            {k("productSearch")}
          </Button>
        </form>
      </div>
      {!source && (
        <label className="knowledge-upload">
          {k("upload")}
          <input
            type="file"
            accept=".pdf,.txt,.md"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f && f.size > 2 * 1024 * 1024) {
                setError(k("uploadHint"));
                return;
              }
              setFile(f);
            }}
          />
          <span className="knowledge-upload-choice">
            <Icon name="plus" />
            {file?.name ?? k("upload")}
          </span>
          <small>{k("uploadHint")}</small>
        </label>
      )}
      {error && <Notice tone="danger">{error}</Notice>}
      <footer className="knowledge-editor-footer">
        <p>{k("saveHint")}</p>
        <Button
          tone="primary"
          type="submit"
          disabled={
            busy || !title[base]?.trim() || (!file && !content[base]?.trim())
          }
          onClick={() => void save()}
        >
          {k("save")}
        </Button>
      </footer>
    </Card>
  );
}
