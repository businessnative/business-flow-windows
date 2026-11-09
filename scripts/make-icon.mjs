import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";

const size = 1024;
const raw = Buffer.alloc((size * 4 + 1) * size);
const bg = [0x0a, 0x0a, 0x0b, 0xff];
const fg = [0xf6, 0xf3, 0xec, 0xff];
const circles = [
  [1.4, 1.4], [1.4, 4.6], [1.4, 7.8], [1.4, 11],
  [5.4, 3], [5.4, 6.2], [5.4, 9.4],
  [9.4, 4.6], [9.4, 7.8], [13.4, 6.2],
];
const scale = size / 15;
const r = 1.35 * scale;

function crc32(buf) {
  let c = ~0;
  for (const b of buf) {
    c ^= b;
    for (let i = 0; i < 8; i += 1) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type), data]);
  const sum = Buffer.alloc(4);
  sum.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, sum]);
}

for (let y = 0; y < size; y += 1) {
  const row = y * (size * 4 + 1);
  raw[row] = 0;
  for (let x = 0; x < size; x += 1) {
    let color = bg;
    for (const [cx, cy] of circles) {
      const dx = x - cx * scale;
      const dy = y - cy * scale;
      if (dx * dx + dy * dy <= r * r) color = fg;
    }
    const o = row + 1 + x * 4;
    raw[o] = color[0];
    raw[o + 1] = color[1];
    raw[o + 2] = color[2];
    raw[o + 3] = color[3];
  }
}
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(size, 0);
ihdr.writeUInt32BE(size, 4);
ihdr[8] = 8;
ihdr[9] = 6;
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", Buffer.alloc(0)),
]);
mkdirSync("src-tauri/icons", { recursive: true });
writeFileSync("src-tauri/icons/source.png", png);
console.log("wrote src-tauri/icons/source.png");
