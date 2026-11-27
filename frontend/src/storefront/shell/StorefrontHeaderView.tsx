/** Shared native shop header markup; callers supply authorized commands or a read-only preview. Narrow screens get one compact row and a menu drawer. */
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { collectionURL } from "../../../../components/catalog/ui/storefront/catalog/product-url";
import Icon from "../../shared/ui/Icon";
import { useShopText } from "../../shared/i18n/shop-i18n";
import { useWorkbenchText } from "../../shared/i18n/workbench-i18n";
import "../styles/mobile-header.css";
export default function StorefrontHeaderView({
  displayName,
  collectionLayout = false,
  company = {},
  children,
  navigation,
  busy = false,
  business = false,
  businessDisabled = false,
  adaptation = false,
  count = 0,
  query = "",
  onSearch,
  onBusiness,
  onAccount,
  onMerchant,
  onConsent,
  onBag,
}: {
  displayName: string;
  collectionLayout?: boolean;
  company?: Record<string, string>;
  children?: ReactNode;
  navigation?: ReactNode;
  busy?: boolean;
  business?: boolean;
  businessDisabled?: boolean;
  adaptation?: boolean;
  count?: number;
  query?: string;
  onSearch?: (query: string) => void;
  onBusiness?: () => void;
  onAccount?: () => void;
  onMerchant?: () => void;
  onConsent?: () => void;
  onBag?: () => void;
}) {
  const { s } = useShopText(),
    { w } = useWorkbenchText();
  const [menu, setMenu] = useState(false),
    [search, setSearch] = useState(false),
    menuId = useId(),
    searchId = useId(),
    menuButton = useRef<HTMLButtonElement>(null);
  const close = () => {
    setMenu(false);
    menuButton.current?.focus();
  };
  useEffect(() => {
    if (!menu) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [menu]);
  // Choosing a destination inside the drawer closes it; the drawer is a menu, not a page.
  const chosen = (action?: () => void) => () => {
    setMenu(false);
    action?.();
  };
  return (
    <header className="shop-nav" data-menu={menu ? "open" : undefined}>
      <button
        ref={menuButton}
        type="button"
        className="shop-menu-button"
        aria-expanded={menu}
        aria-controls={menuId}
        aria-label={s(menu ? "closeMenu" : "menu")}
        onClick={() => (menu ? close() : setMenu(true))}
      >
        <Icon name={menu ? "close" : "menu"} size={22} />
      </button>
      <a href={collectionURL()} className="shop-brand">
        {company.logoUrl && (
          <img
            src={company.logoUrl}
            alt={displayName}
            className="company-brand-logo"
          />
        )}
        {displayName}
        <span> / </span>
      </a>
      <div
        id={menuId}
        className="shop-nav-menu"
        onClick={(e) => {
          if (e.target === e.currentTarget) close();
        }}
      >
        <div className="shop-nav-menu-panel">
          <nav
            onClick={(e) => {
              if ((e.target as Element).closest("a")) setMenu(false);
            }}
          >
            <a href={collectionURL() + (collectionLayout ? "collection" : "")}>
              {s("collection")}
            </a>
            {navigation}
            <button
              disabled={busy || businessDisabled || !onBusiness}
              onClick={chosen(onBusiness)}
            >
              {business ? s("businessAccount") : s("business")}
            </button>
            <button disabled={!onAccount} onClick={chosen(onAccount)}>
              {w("account")}
            </button>
            <button disabled={!onMerchant} onClick={chosen(onMerchant)}>
              {s("studio")} ↗
            </button>
          </nav>
          {children && <div className="shop-nav-settings">{children}</div>}
          <button
            className="shop-text-button"
            aria-pressed={adaptation}
            disabled={!onConsent}
            onClick={chosen(onConsent)}
          >
            {w(adaptation ? "adaptationOn" : "adaptationOff")}
          </button>
        </div>
      </div>
      <button
        type="button"
        className="shop-search-button"
        aria-expanded={search}
        aria-controls={searchId}
        aria-label={s("openSearch")}
        disabled={!onSearch}
        onClick={() => setSearch((open) => !open)}
      >
        <Icon name="search" size={20} />
      </button>
      <button
        className="shop-bag-button"
        disabled={busy || !onBag}
        onClick={onBag}
        aria-label={`${s("bag")} ${count}`}
      >
        <Icon name="box" size={18} />
        <span className="shop-bag-label">{s("bag")}</span> <b>{count}</b>
      </button>
      {search && onSearch && (
        <form
          id={searchId}
          role="search"
          className="shop-header-search"
          onSubmit={(e) => {
            e.preventDefault();
            const field = new FormData(e.currentTarget).get("q");
            onSearch(typeof field === "string" ? field.trim() : "");
            setSearch(false);
          }}
        >
          <input
            name="q"
            type="search"
            defaultValue={query}
            aria-label={s("search")}
            placeholder={s("search")}
            autoFocus
          />
          <button type="submit" aria-label={s("search")}>
            <Icon name="arrow" size={18} />
          </button>
        </form>
      )}
    </header>
  );
}
