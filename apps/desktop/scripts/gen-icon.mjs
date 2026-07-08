// EVEPass app icon: macOS squircle + purple gradient + white keyhole.
// Rendered at 2x and box-downsampled for anti-aliasing. Outputs 1024x1024 RGBA PNG.
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const OUT = process.argv[2];
const W = 1024;
const SS = 2;
const B = W * SS; // supersample canvas

// palette
const top = [0x8b, 0x7f, 0xff];
const bot = [0x53, 0x43, 0xe6];
const mix = (a, b, t) => Math.round(a + (b - a) * t);

// squircle (superellipse) params — leaves a little padding like native icons
const cx = B / 2, cy = B / 2;
// Apple's macOS icon grid: rounded body ≈ 80% of the canvas (≈10% padding).
const R = B * 0.402;
const N = 4.2; // corner roundness (superellipse exponent)

function inSquircle(x, y) {
  const dx = Math.abs(x - cx) / R;
  const dy = Math.abs(y - cy) / R;
  return Math.pow(dx, N) + Math.pow(dy, N) <= 1;
}

// keyhole: a circle (bow) + a trapezoid stem, centered
const kcx = B / 2;
const kcy = B * 0.435;
const kr = B * 0.135; // bow radius
const stemTop = kcy + kr * 0.35;
const stemBot = B * 0.66;
const stemTopHalf = kr * 0.42;
const stemBotHalf = kr * 0.72;

function inKeyhole(x, y) {
  // bow
  const dx = x - kcx, dy = y - kcy;
  if (dx * dx + dy * dy <= kr * kr) return true;
  // stem (trapezoid, wider at bottom)
  if (y >= stemTop && y <= stemBot) {
    const t = (y - stemTop) / (stemBot - stemTop);
    const half = stemTopHalf + (stemBotHalf - stemTopHalf) * t;
    if (Math.abs(x - kcx) <= half) return true;
  }
  return false;
}

// big RGBA buffer
const big = new Uint8Array(B * B * 4);
for (let y = 0; y < B; y++) {
  const gt = y / B;
  // vertical gradient base
  let r = mix(top[0], bot[0], gt);
  let g = mix(top[1], bot[1], gt);
  let b = mix(top[2], bot[2], gt);
  for (let x = 0; x < B; x++) {
    // soft diagonal highlight toward top-left
    const hl = Math.max(0, 1 - Math.hypot(x - B * 0.32, y - B * 0.28) / (B * 0.7));
    let rr = Math.min(255, r + hl * 22);
    let gg = Math.min(255, g + hl * 22);
    let bb = Math.min(255, b + hl * 26);

    let a = inSquircle(x, y) ? 255 : 0;
    if (a && inKeyhole(x, y)) {
      rr = 255; gg = 255; bb = 255;
    }
    const p = (y * B + x) * 4;
    big[p] = rr; big[p + 1] = gg; big[p + 2] = bb; big[p + 3] = a;
  }
}

// downsample SSxSS → W (straight average; RGB is defined everywhere so edges stay clean)
const raw = Buffer.alloc(W * (1 + W * 4));
for (let y = 0; y < W; y++) {
  const rowStart = y * (1 + W * 4);
  raw[rowStart] = 0; // filter: none
  for (let x = 0; x < W; x++) {
    let r = 0, g = 0, bl = 0, a = 0;
    for (let sy = 0; sy < SS; sy++)
      for (let sx = 0; sx < SS; sx++) {
        const p = ((y * SS + sy) * B + (x * SS + sx)) * 4;
        r += big[p]; g += big[p + 1]; bl += big[p + 2]; a += big[p + 3];
      }
    const n = SS * SS;
    const o = rowStart + 1 + x * 4;
    raw[o] = Math.round(r / n);
    raw[o + 1] = Math.round(g / n);
    raw[o + 2] = Math.round(bl / n);
    raw[o + 3] = Math.round(a / n);
  }
}

// ── PNG encode (RGBA) ──
const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const tb = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(Buffer.concat([tb, data])));
  return Buffer.concat([len, tb, data, crc]);
};
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0); ihdr.writeUInt32BE(W, 4);
ihdr[8] = 8; ihdr[9] = 6; // depth 8, RGBA
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", Buffer.alloc(0)),
]);
writeFileSync(OUT, png);
console.log(`wrote ${OUT} (${png.length} bytes)`);
