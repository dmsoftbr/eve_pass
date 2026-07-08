// Menu-bar tray icons: a padlock (locked = closed shackle, unlocked = lifted).
// Template images: black RGB + alpha coverage; macOS tints for light/dark bars.
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const W = 44, SS = 4, B = W * SS;

// geometry (in W space)
const cx = 22, ringCy = 15, Ro = 8, Ri = 5;
const bodyTop = 22, bodyBot = 38, bodyLeft = 11, bodyRight = 33, br = 3.5;
const pivotX = cx - 6.5, pivotY = bodyTop; // left-leg base

function body(x, y) {
  if (x < bodyLeft || x > bodyRight || y < bodyTop || y > bodyBot) return false;
  // rounded corners
  const rx = Math.min(x - bodyLeft, bodyRight - x);
  const ry = Math.min(y - bodyTop, bodyBot - y);
  if (rx < br && ry < br) return (br - rx) ** 2 + (br - ry) ** 2 <= br * br;
  return true;
}

function shackle(x, y) {
  const dx = x - cx, dy = y - ringCy, r = Math.hypot(dx, dy);
  if (dy <= 0 && r >= Ri && r <= Ro) return true; // upper-half ring
  if (y >= ringCy && y <= bodyTop) {
    if (x >= cx - Ro && x <= cx - Ri) return true; // left leg
    if (x >= cx + Ri && x <= cx + Ro) return true; // right leg
  }
  return false;
}

function shackleRotated(x, y, theta) {
  const c = Math.cos(theta), s = Math.sin(theta);
  const dx = x - pivotX, dy = y - pivotY;
  const rx = pivotX + dx * c - dy * s;
  const ry = pivotY + dx * s + dy * c;
  return shackle(rx, ry);
}

function render(openTheta) {
  const raw = Buffer.alloc(W * (1 + W * 4));
  for (let y = 0; y < W; y++) {
    const rowStart = y * (1 + W * 4);
    raw[rowStart] = 0;
    for (let x = 0; x < W; x++) {
      // supersample coverage
      let cov = 0;
      for (let sy = 0; sy < SS; sy++)
        for (let sx = 0; sx < SS; sx++) {
          const px = x + (sx + 0.5) / SS;
          const py = y + (sy + 0.5) / SS;
          const inShackle = openTheta === 0 ? shackle(px, py) : shackleRotated(px, py, openTheta);
          if (body(px, py) || inShackle) cov++;
        }
      const a = Math.round((cov / (SS * SS)) * 255);
      const o = rowStart + 1 + x * 4;
      raw[o] = 0; raw[o + 1] = 0; raw[o + 2] = 0; raw[o + 3] = a; // black + alpha
    }
  }
  return raw;
}

// PNG encode (RGBA)
const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; }
  return t;
})();
const crc32 = (buf) => { let c = 0xffffffff; for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
const chunk = (type, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const tb = Buffer.from(type, "ascii"); const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(Buffer.concat([tb, data]))); return Buffer.concat([len, tb, data, crc]); };
function png(raw) {
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(W, 0); ihdr.writeUInt32BE(W, 4); ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

const dir = process.argv[2];
writeFileSync(`${dir}/tray-locked.png`, png(render(0)));
writeFileSync(`${dir}/tray-unlocked.png`, png(render(0.7)));
console.log("wrote tray-locked.png + tray-unlocked.png");
