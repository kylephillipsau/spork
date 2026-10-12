#!/usr/bin/env node
/**
 * EVERY ICON, FROM THE ONE MARK.
 *
 * The Rack S (`app/shell/rackS.ts`) drawn at each size a browser, a phone or
 * a desktop asks for, in a headless browser so each is rendered at its own
 * pixels rather than scaled from another.
 *
 * The web, in `public/`:
 *  - `favicon.svg` and `favicon.ico` at 16, 32 and 48, the smallest drawn to
 *    whole pixels;
 *  - `apple-touch-icon.png`, 180 and square: iOS rounds its own corners;
 *  - `icon-192.png` and `icon-512.png` for Android and the web manifest, and
 *    `icon-maskable-512.png` with the face inside the circle a launcher may
 *    crop it to; and `manifest.webmanifest`.
 *
 * The native app, in `../mobile/src-tauri/icons/`: every PNG already there
 * drawn again at its own size, in its platform's form (iOS square, Android's
 * legacy, round and adaptive foreground, the rest the rounded tile), its
 * `icon.ico` and `icon.icns`, and the adaptive icon's background colour.
 *
 *   npm run icons
 */

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

import { COLOURS, rackPixelSvg, rackSvg } from "../app/shell/rackS.ts";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const PUBLIC = join(ROOT, "public");
const NATIVE = join(ROOT, "..", "mobile", "src-tauri", "icons");

const browser = await chromium.launch();
// Larger than any icon: a browser window has a smallest size, and a 16-pixel one is not that.
const page = await browser.newPage({ viewport: { width: 1100, height: 1100 } });

/** The SVG drawn at `size` pixels square, as PNG bytes, clear where it is not drawn. One at a time: the one page draws one icon at once. */
async function png(svg, size) {
  await page.setContent(`<html><body style="margin:0;background:transparent">${svg.replace("<svg ", `<svg width="${size}" height="${size}" style="display:block" `)}</body></html>`);
  return page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } });
}

/** The mark at a size, in a shape: drawn to whole pixels where that exists, the sheen from 64 pixels. */
const mark = (size, shape = "rounded", inset = 1) =>
  png(shape === "rounded" && (size === 16 || size === 32) ? rackPixelSvg(size) : rackSvg({ shape, inset, texture: size >= 64 }), size);

/** PNGs in one ICO, each entry pointing at its own PNG, as Windows and browsers read them. */
function ico(images) {
  const head = Buffer.alloc(6 + 16 * images.length);
  head.writeUInt16LE(1, 2);
  head.writeUInt16LE(images.length, 4);
  let offset = head.length;
  images.forEach(({ size, data }, i) => {
    const at = 6 + 16 * i;
    head.writeUInt8(size >= 256 ? 0 : size, at);
    head.writeUInt8(size >= 256 ? 0 : size, at + 1);
    head.writeUInt16LE(1, at + 4);
    head.writeUInt16LE(32, at + 6);
    head.writeUInt32LE(data.length, at + 8);
    head.writeUInt32LE(offset, at + 12);
    offset += data.length;
  });
  return Buffer.concat([head, ...images.map((i) => i.data)]);
}

/** PNGs in one ICNS, each under the type macOS reads for its size. */
function icns(images) {
  const entries = images.map(({ type, data }) => {
    const head = Buffer.alloc(8);
    head.write(type, 0, "ascii");
    head.writeUInt32BE(8 + data.length, 4);
    return Buffer.concat([head, data]);
  });
  const head = Buffer.alloc(8);
  head.write("icns", 0, "ascii");
  head.writeUInt32BE(8 + entries.reduce((n, e) => n + e.length, 0), 4);
  return Buffer.concat([head, ...entries]);
}

const write = (path, data) => {
  writeFileSync(path, data);
  console.log(`  ${relative(join(ROOT, ".."), path).split(sep).join("/")}`);
};

// ── the web ───────────────────────────────────────────────────────────
write(join(PUBLIC, "favicon.svg"), rackPixelSvg(16) + "\n");
const favicons = [];
for (const size of [16, 32, 48]) favicons.push({ size, data: await mark(size) });
write(join(PUBLIC, "favicon.ico"), ico(favicons));
write(join(PUBLIC, "apple-touch-icon.png"), await mark(180, "square"));
write(join(PUBLIC, "icon-192.png"), await mark(192));
write(join(PUBLIC, "icon-512.png"), await mark(512));
// A launcher may crop to a circle four-fifths across: the face shrunk to sit inside it.
write(join(PUBLIC, "icon-maskable-512.png"), await mark(512, "square", 1.25));
write(
  join(PUBLIC, "manifest.webmanifest"),
  JSON.stringify(
    {
      name: "Spork",
      short_name: "Spork",
      start_url: "/",
      display: "browser",
      background_color: COLOURS.tile,
      theme_color: COLOURS.tile,
      icons: [
        { src: "/icon-192.png", sizes: "192x192", type: "image/png" },
        { src: "/icon-512.png", sizes: "512x512", type: "image/png" },
        { src: "/icon-maskable-512.png", sizes: "512x512", type: "image/png", purpose: "maskable" },
      ],
    },
    null,
    2,
  ) + "\n",
);

// ── the native app ────────────────────────────────────────────────────
const walk = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)]));
for (const path of walk(NATIVE).filter((p) => p.endsWith(".png"))) {
  const name = relative(NATIVE, path).split(sep).join("/");
  const size = readFileSync(path).readUInt32BE(16);
  // iOS rounds its own corners. Android's adaptive foreground is the face
  // alone inside the 66 of 108 a launcher always shows, over the tile's
  // colour; its round icon a circle with the face inside it.
  const form = name.startsWith("ios/")
    ? ["square"]
    : name.endsWith("ic_launcher_foreground.png")
      ? ["none", 1.6]
      : name.endsWith("ic_launcher_round.png")
        ? ["circle", 1.12]
        : ["rounded"];
  write(path, await mark(size, ...form));
}
const desktop = [];
for (const size of [16, 32, 48, 64, 256]) desktop.push({ size, data: await mark(size) });
write(join(NATIVE, "icon.ico"), ico(desktop));
const mac = [];
for (const [type, size] of [["ic11", 32], ["ic12", 64], ["ic07", 128], ["ic08", 256], ["ic13", 256], ["ic09", 512], ["ic14", 512], ["ic10", 1024]]) mac.push({ type, data: await mark(size) });
write(join(NATIVE, "icon.icns"), icns(mac));
write(
  join(NATIVE, "android", "values", "ic_launcher_background.xml"),
  `<?xml version="1.0" encoding="utf-8"?>\n<resources>\n  <color name="ic_launcher_background">${COLOURS.tile}</color>\n</resources>`,
);

await browser.close();
console.log("\nicons ok");
