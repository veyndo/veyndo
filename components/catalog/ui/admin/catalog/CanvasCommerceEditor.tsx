/** Canvas handoff reuses native product/category editors without unmounting or copying the page designer. */
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import { Button, Notice } from "../../../../../frontend/src/kit/ui";
import "./canvas-commerce-editor.css";
import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import type { ApiRequestFn as RequestFn } from "../../../../../frontend/src/shared/api/types";
import type { Category } from "./catalog-model";
import { useCanvasText } from "../../../../../frontend/src/shared/canvas/canvas-i18n";
import { useCatalogText } from "../../../../../frontend/src/shared/i18n/catalog-i18n";
import ConfirmDialog from "../../../../../frontend/src/shared/ui/ConfirmDialog";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
const ProductEditor = lazy(() => import("./ProductEditor"));
const CategoriesWorkspace = lazy(() => import("./CategoriesWorkspace"));
export default function CanvasCommerceEditor({
  kind,
  id,
  request,
  canWrite,
  onBack,
  onSaved,
  onDirtyChange,
}: {
  kind: "product" | "category";
  id: string;
  request: RequestFn;
  canWrite: boolean;
  onBack: () => void;
  onSaved: () => void;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const t = useCanvasText(),
    { c } = useCatalogText();
  const [dirty, setDirty] = useState(false),
    [leaving, setLeaving] = useState(false);
  const categoriesPath = "/api/merchant/categories";
  const categoriesRead = useRead(request, categoriesPath);
  const setReadCategories = useSetRead(request);
  const categories: Category[] | undefined = categoriesRead.data?.elements;
  const categoriesLoadError = (categoriesRead.error as Error | undefined)
    ?.message;

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);
  const reload = useCallback(async () => {
    setReadCategories(categoriesPath, await request(categoriesPath));
    onSaved();
  }, [request, onSaved]);
  return (
    <section className="canvas-commerce-editor">
      <header className="canvas-commerce-heading">
        <Button
          type="button"
          onClick={() => (dirty ? setLeaving(true) : onBack())}
        >
          <Icon name="chevron-left" size={16} />
          {t("backToPages")}
        </Button>
        <div>
          <h2>{t(kind === "product" ? "productData" : "categoryData")}</h2>
          <p>{t("dataEditorHint")}</p>
        </div>
        {!canWrite && (
          <span className="canvas-commerce-readonly">
            <Icon name="lock" size={16} />
            {t("dataReadOnly")}
          </span>
        )}
      </header>
      {categoriesLoadError && (
        <Notice tone="danger">{categoriesLoadError}</Notice>
      )}
      <Suspense fallback={<p role="status">{c("loading")}</p>}>
        {categories &&
          (kind === "product" ? (
            <ProductEditor
              id={id}
              backLabel={t("backToPages")}
              categories={categories}
              request={request}
              canWrite={canWrite}
              onBack={onBack}
              onCreated={onSaved}
              onDirtyChange={setDirty}
            />
          ) : (
            <CategoriesWorkspace
              initialId={id}
              categories={categories}
              request={request}
              canWrite={canWrite}
              onRefresh={() => void reload()}
              onDirtyChange={setDirty}
            />
          ))}
      </Suspense>
      {leaving && (
        <ConfirmDialog
          title={c("unsaved")}
          confirmLabel={c("discard")}
          onCancel={() => setLeaving(false)}
          onConfirm={onBack}
        >
          <p>{c("unsaved")}</p>
        </ConfirmDialog>
      )}
    </section>
  );
}
