# admin/apps (vendored import path)

The hash-locked Marketplace Studio app (`extensions/apps/marketplaces`, pinned in
`components.lock.json`) imports this path. The module re-exports its owner in
`components/`; it goes away when that app is next imported with the new path.
