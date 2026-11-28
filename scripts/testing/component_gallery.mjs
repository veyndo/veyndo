// Renders the component gallery in Chromium: axe (including contrast) per language, and a visual snapshot of every example frame.
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";

const { chromium } = await import(process.env.BROWSER_MODULE);
const url = process.env.GALLERY_URL;
const baselines = process.env.GALLERY_BASELINES;
const out = process.env.GALLERY_ARTIFACTS;
const update = process.env.GALLERY_UPDATE === "1";
const axeSource = readFileSync(process.env.AXE_PATH, "utf8");
// A pixel differs when a channel moves by more than this; a frame fails above the ratio.
const channel = 40,
  ratio = 0.01;
mkdirSync(out, { recursive: true });

const browser = await chromium.launch();
const failures = [];
let compared = 0,
  recorded = 0;
try {
  for (const locale of ["en-GB", "de-DE"]) {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 900 },
      deviceScaleFactor: 1,
      reducedMotion: "reduce",
      locale,
    });
    page.on("pageerror", (error) => failures.push(`${locale}: page error ${error.message}`));
    await page.goto(`${url}?lang=${locale}`);
    await page.waitForSelector("[data-gallery-example]");
    // Both environments render with the same installed font (Playwright's Chromium dependencies).
    await page.addStyleTag({
      content: '*{font-family:"Liberation Sans",sans-serif!important;caret-color:transparent!important}',
    });
    await page.evaluate(() => document.fonts.ready);
    await page.addScriptTag({ content: axeSource });
    const axe = await page.evaluate(async () => {
      // Each example appears once per theme, so the same labelled landmark repeats by design.
      const result = await window.axe.run(document, {
        resultTypes: ["violations"],
        rules: { "landmark-unique": { enabled: false } },
      });
      return result.violations.map((v) => `${v.id} (${v.impact}): ${v.nodes.slice(0, 3).map((n) => n.target.join(" ")).join(" | ")}`);
    });
    for (const violation of axe) failures.push(`${locale}: axe ${violation}`);
    const refused = await page.locator("[data-refusal]").count();
    if (refused) failures.push(`${locale}: ${refused} gallery examples were refused`);
    const frames = page.locator("[data-gallery-example]");
    for (let i = 0; i < (await frames.count()); i++) {
      const frame = frames.nth(i);
      const id = (await frame.getAttribute("data-gallery-example")).replaceAll("/", "--");
      const name = `${id}--${locale}.png`;
      const actual = await frame.screenshot({ animations: "disabled" });
      const baseline = path.join(baselines, name);
      if (update || !existsSync(baseline)) {
        writeFileSync(path.join(out, name), actual);
        if (update) {
          mkdirSync(baselines, { recursive: true });
          writeFileSync(baseline, actual);
          recorded++;
        } else failures.push(`${name}: no baseline (recorded in ${out}; commit it after review)`);
        continue;
      }
      compared++;
      // Decoded and compared in the page, so the check needs no image library.
      const diff = await page.evaluate(
        async ([a, b, limit]) => {
          const load = async (data) => createImageBitmap(await (await fetch(`data:image/png;base64,${data}`)).blob());
          const [x, y] = await Promise.all([load(a), load(b)]);
          if (x.width !== y.width || x.height !== y.height) return { size: `${x.width}x${x.height} vs ${y.width}x${y.height}` };
          const read = (image) => {
            const canvas = new OffscreenCanvas(image.width, image.height);
            const context = canvas.getContext("2d");
            context.drawImage(image, 0, 0);
            return context.getImageData(0, 0, image.width, image.height).data;
          };
          const [p, q] = [read(x), read(y)];
          let changed = 0;
          for (let i = 0; i < p.length; i += 4)
            if (Math.max(Math.abs(p[i] - q[i]), Math.abs(p[i + 1] - q[i + 1]), Math.abs(p[i + 2] - q[i + 2])) > limit) changed++;
          return { changed: changed / (p.length / 4) };
        },
        [readFileSync(baseline).toString("base64"), actual.toString("base64"), channel],
      );
      if (diff.size || diff.changed > ratio) {
        writeFileSync(path.join(out, name), actual);
        failures.push(`${name}: ${diff.size ? `size ${diff.size}` : `${(diff.changed * 100).toFixed(2)}% pixels changed`} (actual in ${out})`);
      }
    }
    await page.close();
  }
} finally {
  await browser.close();
}
console.log(`component gallery: ${compared} snapshots compared, ${recorded} recorded`);
if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
