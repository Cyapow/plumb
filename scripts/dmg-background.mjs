// Renders the DMG installer background (1x + @2x) from an SVG, in Plumb's
// default (Modernist Dark) palette. Run: node scripts/dmg-background.mjs
import { Resvg } from "@resvg/resvg-js";
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const W = 660;
const H = 400;

// Plumb Modernist Dark tokens.
const BG = "#171616";
const BG2 = "#201e1d";
const LINE = "#3a3736";
const TEXT = "#f3f2f2";
const DIM = "#8c8888";
const ACCENT = "#ff563c";

// Icon slots (top-left origin, matching the bundle window layout).
const APP = { x: 180, y: 175 };
const APPS = { x: 480, y: 175 };
const SLOT = 150; // drop-zone square

// A faint grid.
let grid = "";
for (let x = 40; x < W; x += 40) grid += `<line x1="${x}" y1="0" x2="${x}" y2="${H}"/>`;
for (let y = 40; y < H; y += 40) grid += `<line x1="0" y1="${y}" x2="${W}" y2="${y}"/>`;

// Corner brackets.
const bracket = (x, y, sx, sy) =>
  `<path d="M ${x + sx * 26} ${y} H ${x} V ${y + sy * 26}" fill="none" stroke="${LINE}" stroke-width="2"/>`;

// The Plumb mark (line + node + bob) as a subtle watermark, bottom-right.
const mark = (cx, cy, s, color, op) => {
  const line = `<rect x="${cx - s * 0.06}" y="${cy - s * 0.5}" width="${s * 0.12}" height="${s * 0.72}" fill="${color}"/>`;
  const node = `<rect x="${cx - s * 0.2}" y="${cy - s * 0.26}" width="${s * 0.4}" height="${s * 0.4}" fill="${color}"/>`;
  const bob = `<polygon points="${cx - s * 0.5},${cy + s * 0.22} ${cx + s * 0.5},${cy + s * 0.22} ${cx},${cy + s * 0.62}" fill="${color}"/>`;
  return `<g opacity="${op}">${line}${node}${bob}</g>`;
};

const dropZone = (c) =>
  `<rect x="${c.x - SLOT / 2}" y="${c.y - SLOT / 2}" width="${SLOT}" height="${SLOT}" rx="22"
     fill="${BG2}" fill-opacity="0.5" stroke="${LINE}" stroke-width="1.5" stroke-dasharray="6 6"/>`;

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <defs>
    <radialGradient id="vig" cx="50%" cy="38%" r="75%">
      <stop offset="0%" stop-color="${BG2}"/>
      <stop offset="100%" stop-color="${BG}"/>
    </radialGradient>
  </defs>
  <rect width="${W}" height="${H}" fill="url(#vig)"/>
  <g stroke="${LINE}" stroke-width="1" opacity="0.18">${grid}</g>
  ${bracket(24, 24, 1, 1)} ${bracket(W - 24, 24, -1, 1)}
  ${bracket(24, H - 24, 1, -1)} ${bracket(W - 24, H - 24, -1, -1)}

  ${mark(W - 70, H - 62, 60, LINE, 0.5)}

  <text x="${W / 2}" y="70" text-anchor="middle" fill="${TEXT}"
    font-family="-apple-system, Helvetica Neue, Arial, sans-serif" font-size="26" font-weight="700"
    letter-spacing="0.3">Install Plumb</text>
  <text x="${W / 2}" y="98" text-anchor="middle" fill="${DIM}"
    font-family="-apple-system, Helvetica Neue, Arial, sans-serif" font-size="13">
    Drag the app onto the Applications folder</text>

  ${dropZone(APP)} ${dropZone(APPS)}

  <!-- Arrow between the two slots -->
  <g stroke="${ACCENT}" stroke-width="4" fill="none" stroke-linecap="round" stroke-linejoin="round">
    <line x1="300" y1="${APP.y}" x2="356" y2="${APP.y}"/>
    <polyline points="344,${APP.y - 12} 360,${APP.y} 344,${APP.y + 12}"/>
  </g>
</svg>`;

function render(scale, out) {
  const r = new Resvg(svg, { fitTo: { mode: "width", value: W * scale } });
  writeFileSync(join(root, out), r.render().asPng());
  console.log("wrote", out, `${W * scale}x${H * scale}`);
}

render(1, "src-tauri/dmg-background.png");
render(2, "src-tauri/dmg-background@2x.png");
