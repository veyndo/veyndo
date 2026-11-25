/** Enforce source ownership, bounded modules and acyclic runtime dependencies using TypeScript ASTs. */
import fs from "node:fs";
import path from "node:path";
import ts from "typescript";
import { tokenDrift } from "./tokens.mjs";
const root = path.resolve(import.meta.dirname, "..");
const source = path.resolve(root, process.argv[2] ?? "src");
const walk = (dir) =>
  fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((e) =>
      e.isDirectory() ? walk(path.join(dir, e.name)) : [path.join(dir, e.name)],
    );
const appRoot = path.resolve(root, "../extensions/apps");
const pluginFiles = process.argv[2]
  ? []
  : walk(appRoot).filter(
      (f) => /\.(tsx?|css)$/.test(f) && !f.includes("/tests/"),
    );
const files = [
  ...walk(source).filter((f) => /\.(tsx?|css)$/.test(f)),
  ...pluginFiles,
];
const relative = (f) => path.relative(source, f).replaceAll("\\", "/");
const errors = [];
const edges = new Map();
const owners = [
  "admin",
  "storefront",
  "platform",
  "shared",
  "application",
  "kit",
];
const owner = (f) => relative(f).split("/")[0];
// kit packages: a package imports only the packages listed here; outsiders import only its index.
const kitLayers = {
  tokens: [],
  ui: ["tokens"],
  api: [],
  query: [],
  storefront: ["tokens", "ui", "api", "query"],
};
const kitPackage = (f) =>
  owner(f) === "kit" ? relative(f).split("/")[1] : undefined;
// Hash-locked vendored components (components.lock.json) change only by explicit import.
const lockedRoots = Object.values(
  JSON.parse(
    fs.readFileSync(path.resolve(root, "../components.lock.json"), "utf8"),
  ).components,
).map((row) => path.resolve(root, "..", row.path) + path.sep);
const vendored = (file) =>
  lockedRoots.some((prefix) => file.startsWith(prefix));
const colourLiteral = /#[0-9a-fA-F]{3,8}\b|\b(?:rgb|rgba|hsl|hsla)\(/;
const resolve = (from, spec) => {
  const base = path.resolve(path.dirname(from), spec);
  return [
    base,
    ...[".ts", ".tsx", ".css", ".js", "/index.ts", "/index.tsx"].map(
      (ext) => base + ext,
    ),
  ].find((f) => fs.existsSync(f) && fs.statSync(f).isFile());
};
for (const file of files) {
  const text = fs.readFileSync(file, "utf8");
  if (!text.startsWith("/**") && !text.startsWith("/*"))
    errors.push(`${relative(file)} needs a leading responsibility comment`);
  if (path.dirname(file) === source && path.basename(file) !== "main.tsx")
    errors.push(`${relative(file)} belongs in a feature folder`);
  // Studio areas style with design tokens and kit parts only (F4).
  if (
    file.endsWith(".css") &&
    owner(file) === "admin" &&
    colourLiteral.test(text.replace(/\/\*[\s\S]*?\*\//g, ""))
  )
    errors.push(
      `${relative(file)} uses a colour literal; Studio styles use var(--vy-color-*)`,
    );
  if (
    /\.tsx?$/.test(file) &&
    !vendored(file) &&
    (["admin", "shared"].includes(owner(file)) ||
      (file.startsWith(appRoot + path.sep) &&
        file.includes(`${path.sep}studio${path.sep}`))) &&
    /["'`\s]studio-(?:primary|secondary|card)["'`\s]/.test(text)
  )
    errors.push(
      `${relative(file)} uses a retired Studio class; use kit Button, LinkButton or Card`,
    );
  if (
    file.endsWith(".css") &&
    kitPackage(file) &&
    !file.endsWith("/tokens.css") &&
    colourLiteral.test(text.replace(/\/\*[\s\S]*?\*\//g, ""))
  )
    errors.push(
      `${relative(file)} uses a colour literal; use var(--vy-color-*)`,
    );
  if (file.endsWith(".css")) continue;
  const ast = ts.createSourceFile(
    file,
    text,
    ts.ScriptTarget.Latest,
    true,
    file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
  const links = [];
  // A Studio extension descriptor registers its views with lazy import(); those
  // loaders run only when the route opens, so they are not initialisation edges.
  const registers = path.basename(file) === "studio-extension.ts";
  let registration = false;
  const add = (spec, runtime) => {
    if (!spec.startsWith(".")) return;
    const target = resolve(file, spec);
    if (!target) {
      errors.push(`${relative(file)} cannot resolve ${spec}`);
      return;
    }
    if (runtime && target.includes("/tests/"))
      errors.push(`${relative(file)} imports test code into production`);
    if (
      target.endsWith("/manifest.json") &&
      target.startsWith(appRoot + path.sep)
    )
      return;
    if (
      target ===
      path.resolve(root, "../extensions/sdk/native-asset-actions.json")
    )
      return; // Immutable host/SDK contract shared with the Rust manifest compiler.
    if (target === path.resolve(root, "../extensions/sdk/analytics.js")) return; // Explicit public SDK boundary, contract-tested separately.
    if (
      !target.startsWith(source + path.sep) &&
      !target.startsWith(appRoot + path.sep)
    ) {
      errors.push(
        `${relative(file)} imports outside the source ownership tree: ${spec}`,
      );
      return;
    }
    if (
      file.startsWith(appRoot + path.sep) &&
      target.startsWith(appRoot + path.sep) &&
      path.relative(appRoot, file).split(path.sep)[0] !==
        path.relative(appRoot, target).split(path.sep)[0]
    ) {
      errors.push(
        `${relative(file)} imports another app's implementation: ${spec}`,
      );
      return;
    }
    const a = owner(file),
      b = owner(target);
    if (a === "kit" && b !== "kit" && owners.includes(b))
      errors.push(
        `${relative(file)} imports outside kit via ${relative(target)}`,
      );
    const from = kitPackage(file),
      to = kitPackage(target);
    if (from && to && from !== to && !(kitLayers[from] ?? []).includes(to))
      errors.push(
        `${relative(file)} breaks the kit layering (${from} may not import ${to})`,
      );
    if (to && from !== to && !/\/kit\/[^/]+\/index\.tsx?$/.test(target))
      errors.push(
        `${relative(file)} imports kit/${to} internals; import its index`,
      );
    if (a === "shared" && owners.includes(b) && !["shared", "kit"].includes(b))
      errors.push(
        `${relative(file)} violates shared boundary via ${relative(target)}`,
      );
    if (
      ["admin", "storefront", "platform"].includes(a) &&
      owners.includes(b) &&
      a !== b &&
      b !== "shared" &&
      b !== "kit"
    )
      errors.push(
        `${relative(file)} crosses application boundary via ${relative(target)}`,
      );
    if (runtime && !target.endsWith(".css") && !registration)
      links.push(target);
  };
  const visit = (node) => {
    if (
      ts.isImportDeclaration(node) &&
      ts.isStringLiteral(node.moduleSpecifier)
    ) {
      const clause = node.importClause;
      const runtime =
        !clause ||
        (!clause.isTypeOnly &&
          (!!clause.name ||
            !clause.namedBindings ||
            ts.isNamespaceImport(clause.namedBindings) ||
            clause.namedBindings.elements.some((e) => !e.isTypeOnly)));
      add(node.moduleSpecifier.text, runtime);
    } else if (
      ts.isExportDeclaration(node) &&
      node.moduleSpecifier &&
      ts.isStringLiteral(node.moduleSpecifier)
    ) {
      const runtime =
        !node.isTypeOnly &&
        (!node.exportClause ||
          !ts.isNamedExports(node.exportClause) ||
          node.exportClause.elements.some((e) => !e.isTypeOnly));
      add(node.moduleSpecifier.text, runtime);
    } else if (
      ts.isCallExpression(node) &&
      node.expression.kind === ts.SyntaxKind.ImportKeyword &&
      node.arguments[0] &&
      ts.isStringLiteral(node.arguments[0])
    ) {
      registration = registers;
      add(node.arguments[0].text, true);
      registration = false;
    } else if (
      ts.isImportTypeNode(node) &&
      ts.isLiteralTypeNode(node.argument) &&
      ts.isStringLiteral(node.argument.literal)
    )
      add(node.argument.literal.text, false);
    ts.forEachChild(node, visit);
  };
  visit(ast);
  edges.set(file, links);
  if (!fs.existsSync(path.join(path.dirname(file), "README.md")))
    errors.push(`${relative(file)} has no folder contract (README.md)`);
}
const done = new Set(),
  stack = [];
function check(file) {
  if (stack.includes(file)) {
    errors.push(
      `Runtime import cycle: ${[...stack.slice(stack.indexOf(file)), file].map(relative).join(" -> ")}`,
    );
    return;
  }
  if (done.has(file)) return;
  stack.push(file);
  for (const target of edges.get(file) || []) check(target);
  stack.pop();
  done.add(file);
}
for (const file of edges.keys()) check(file);
if (!process.argv[2]) {
  const drift = tokenDrift();
  if (drift) errors.push(drift);
  // The handwritten admin stylesheet folder only shrinks; screens move to kit/ui instead.
  const budgetFile = path.join(root, "scripts/admin-styles-budget.json");
  const budget = JSON.parse(fs.readFileSync(budgetFile, "utf8"));
  const sheets = walk(path.join(source, "admin/styles")).filter((f) =>
    f.endsWith(".css"),
  );
  const lines = sheets.reduce(
    (sum, f) => sum + fs.readFileSync(f, "utf8").split("\n").length - 1,
    0,
  );
  // Moving a sheet into its area does not count as removal: all admin CSS only falls too.
  const adminLines = walk(path.join(source, "admin"))
    .filter((f) => f.endsWith(".css"))
    .reduce(
      (sum, f) => sum + fs.readFileSync(f, "utf8").split("\n").length - 1,
      0,
    );
  if (adminLines > budget.adminLines)
    errors.push(
      `admin CSS grew to ${adminLines} lines (budget ${budget.adminLines}); new screens use kit/ui`,
    );
  else if (adminLines < budget.adminLines)
    errors.push(
      `admin CSS shrank to ${adminLines} lines; lower adminLines in scripts/admin-styles-budget.json to match`,
    );
  // Studio areas read server data through kit/query (useRead); effects that call
  // the request function directly only fall until none is left.
  let effectReads = 0;
  for (const file of walk(path.join(source, "admin")).filter((f) =>
    /\.tsx?$/.test(f),
  )) {
    const text = fs.readFileSync(file, "utf8");
    for (const match of text.matchAll(/useEffect\(\s*\(\)\s*=>\s*\{/g)) {
      let depth = 1,
        index = match.index + match[0].length;
      while (depth && index < text.length) {
        if (text[index] === "{") depth++;
        else if (text[index] === "}") depth--;
        index++;
      }
      const body = text.slice(match.index, index);
      // Writes such as autosave are effects by nature; they say so and why.
      if (
        /\b(?:request|liveRequest|requestJson)\(/.test(body) &&
        !/write-effect: \S/.test(body)
      )
        effectReads++;
    }
  }
  if (effectReads > budget.effectReads)
    errors.push(
      `${effectReads} Studio effects call the request function (budget ${budget.effectReads}); read through useRead`,
    );
  else if (effectReads < budget.effectReads)
    errors.push(
      `Studio effects with direct reads fell to ${effectReads}; lower effectReads in scripts/admin-styles-budget.json`,
    );
  if (sheets.length > budget.files || lines > budget.lines)
    errors.push(
      `admin/styles grew to ${sheets.length} files / ${lines} lines (budget ${budget.files} / ${budget.lines}); new screens use kit/ui`,
    );
  else if (sheets.length < budget.files || lines < budget.lines)
    errors.push(
      `admin/styles shrank to ${sheets.length} files / ${lines} lines; lower scripts/admin-styles-budget.json to match`,
    );
}
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(
  `PASS ${files.length} frontend modules: ownership, resolved imports, runtime DAG, responsibility comments and folder contracts`,
);
