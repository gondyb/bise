// Width and height of a base64 JPEG, read from its SOF marker. Pure.

export function jpegSize(base64) {
  const bin = atob(base64.slice(0, 200_000));
  const at = (i) => bin.charCodeAt(i);
  if (at(0) !== 0xff || at(1) !== 0xd8) return { width: 0, height: 0 };
  let i = 2;
  while (i + 9 < bin.length) {
    if (at(i) !== 0xff) { i++; continue; }
    const marker = at(i + 1);
    if (marker === 0xff) { i++; continue; }
    const len = (at(i + 2) << 8) | at(i + 3);
    // SOF0..SOF15, not DHT (c4), JPG (c8), DAC (cc).
    if (marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker)) {
      return { height: (at(i + 5) << 8) | at(i + 6), width: (at(i + 7) << 8) | at(i + 8) };
    }
    i += 2 + len;
  }
  return { width: 0, height: 0 };
}
