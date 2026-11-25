/** Frontend source roots: frontend/src plus each component's UI folder (components/<name>/ui), read as one tree. */
import fs from "node:fs";
import path from "node:path";

export const frontendRoot = path.resolve(import.meta.dirname, "..");
export const componentsRoot = path.resolve(frontendRoot, "../components");

/** Every components/<name>/ui folder; each holds `admin/<area>` and `storefront/<area>`. */
export function componentUiRoots() {
  return fs
    .readdirSync(componentsRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(componentsRoot, entry.name, "ui"))
    .filter((dir) => fs.existsSync(dir));
}

/** The component UI root a file lies in, if any. */
export const uiRootOf = (file) =>
  componentUiRoots().find((root) => file.startsWith(root + path.sep));

/**
 * A file's path in the one source tree: `components/orders/ui/admin/orders/X.tsx`
 * reads as `admin/orders/X.tsx`, like files under `frontend/src`.
 */
export function logicalPath(file, source = path.join(frontendRoot, "src")) {
  const root = uiRootOf(file);
  return path.relative(root ?? source, file).replaceAll("\\", "/");
}

export const walk = (dir) =>
  fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) =>
      entry.isDirectory()
        ? walk(path.join(dir, entry.name))
        : [path.join(dir, entry.name)],
    );

/** Files of the whole source tree: `frontend/src` and every component UI folder. */
export const sourceFiles = (source = path.join(frontendRoot, "src")) => [
  ...walk(source),
  ...componentUiRoots().flatMap(walk),
];

/** Files under one application folder (`admin`, `storefront`) across all roots. */
export const appFiles = (app, source = path.join(frontendRoot, "src")) =>
  [source, ...componentUiRoots()]
    .map((root) => path.join(root, app))
    .filter((dir) => fs.existsSync(dir))
    .flatMap(walk);
