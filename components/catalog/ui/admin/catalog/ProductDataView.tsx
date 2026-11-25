/** Central catalog workspace uses shared server-paged grids, existing product editing and hierarchical categories. */
import {
  useInvalidateRead,
  useRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import { Button, Notice } from "../../../../../frontend/src/kit/ui";
import type { GridCriteria } from "../../../../../frontend/src/shared/grid/types";
import { lazy, useEffect, useState } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import type { OpenEntity } from "../../../../../frontend/src/admin/shell/useEntityNavigation";
import { useCatalogText } from "../../../../../frontend/src/shared/i18n/catalog-i18n";
import type { Category } from "./catalog-model";
const ProductEditor = lazy(() => import("./ProductEditor"));
import CatalogPreparation from "./CatalogPreparation";
import { usePreparationText } from "./preparation-i18n";
import CatalogDirectory from "./CatalogDirectory";
import CategoriesWorkspace from "./CategoriesWorkspace";
import "./catalog.css";
import "./catalog-editor.css";
export default function ProductDataView({
  request,
  initialId,
  onEntityBack,
  onEntity,
  canWrite = true,
  onDirtyChange,
}: {
  onDirtyChange?: (dirty: boolean) => void;
  request: RequestFn;
  canWrite?: boolean;
  initialId?: string;
  onEntityBack?: () => void;
  onEntity?: OpenEntity;
}) {
  const [gridCursor, setGridCursor] = useState("");
  const [gridCriteria, setGridCriteria] = useState<GridCriteria>({});
  const { c } = useCatalogText();
  const preparation = usePreparationText();
  const [initialTab, setInitialTab] = useState("general");
  const [view, setView] = useState("products"),
    [id, setId] = useState<string | null>(null),
    categoryRead = useRead(request, "/api/merchant/categories"),
    invalidate = useInvalidateRead(request);
  const categories: Category[] = categoryRead.data?.elements ?? [];
  const mainLocale: string = categoryRead.data?.mainLocale ?? "en-GB";
  const error = (categoryRead.error as Error | undefined)?.message ?? "";
  const refreshCategories = () => invalidate("/api/merchant/categories");
  useEffect(() => setId(initialId ?? null), [initialId]);
  if (id !== null)
    return (
      <ProductEditor
        onDirtyChange={onDirtyChange}
        key={id}
        id={id}
        initialTab={initialTab}
        request={request}
        categories={categories}
        canWrite={canWrite}
        onBack={() => {
          refreshCategories();
          if (onEntityBack) onEntityBack();
          else setId(null);
        }}
        onCreated={(id) => (onEntity ? onEntity("productData", id) : setId(id))}
      />
    );
  return (
    <div className="studio-page catalog-workspace">
      <div className="catalog-heading">
        <div>
          <p className="catalog-eyebrow">{c("products")}</p>
          <h1>{c("products")}</h1>
          <p>{c("intro")}</p>
        </div>
        {view === "products" && canWrite && (
          <Button
            tone="primary"
            type="button"
            onClick={() => {
              setInitialTab("general");
              setId("");
            }}
          >
            + {c("newProduct")}
          </Button>
        )}
      </div>
      <div className="catalog-tabs" role="tablist">
        <button
          role="tab"
          aria-selected={view === "preparation"}
          onClick={() => setView("preparation")}
        >
          {preparation("title")}
        </button>
        <button
          role="tab"
          aria-selected={view === "products"}
          onClick={() => setView("products")}
        >
          {c("products")}
        </button>
        <button
          role="tab"
          aria-selected={view === "categories"}
          onClick={() => setView("categories")}
        >
          {c("categories")}
        </button>
      </div>
      {error && <Notice tone="danger">{error}</Notice>}
      {view === "preparation" ? (
        <CatalogPreparation
          request={request}
          onOpen={(id, tab) => {
            setInitialTab(tab);
            if (onEntity) onEntity("productData", id);
            else setId(id);
          }}
        />
      ) : view === "categories" ? (
        <CategoriesWorkspace
          request={request}
          categories={categories}
          canWrite={canWrite}
          onRefresh={refreshCategories}
        />
      ) : (
        <CatalogDirectory
          queryState={{
            criteria: gridCriteria,
            onCriteria: setGridCriteria,
            cursor: gridCursor,
            onCursor: setGridCursor,
          }}
          request={request}
          categories={categories}
          mainLocale={mainLocale}
          canWrite={canWrite}
          onNew={() => setId("")}
          onOpen={(id) => {
            setInitialTab("general");
            if (onEntity) onEntity("productData", id);
            else setId(id);
          }}
        />
      )}
    </div>
  );
}
