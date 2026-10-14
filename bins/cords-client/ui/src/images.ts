// SVG is accepted only as an inert subset, rendered as an image and rasterized.
// Do not insert source SVG into the application DOM or loosen CSP for image hosts.
const svgElements = new Set([
  'svg',
  'g',
  'path',
  'rect',
  'circle',
  'ellipse',
  'line',
  'polyline',
  'polygon',
]);
const svgAttributes = new Set([
  'xmlns',
  'viewBox',
  'width',
  'height',
  'x',
  'y',
  'x1',
  'y1',
  'x2',
  'y2',
  'cx',
  'cy',
  'r',
  'rx',
  'ry',
  'd',
  'points',
  'fill',
  'stroke',
  'stroke-width',
  'stroke-linecap',
  'stroke-linejoin',
  'fill-rule',
  'clip-rule',
  'opacity',
  'fill-opacity',
  'stroke-opacity',
  'transform',
]);
export function safeSvg(source: string): string {
  if (/<!DOCTYPE|<!ENTITY/i.test(source))
    throw new Error('SVG document declarations are not supported');
  const document = new DOMParser().parseFromString(source, 'image/svg+xml');
  if (document.querySelector('parsererror') || document.documentElement.localName !== 'svg')
    throw new Error('Invalid SVG');
  for (const element of document.querySelectorAll('*')) {
    if (
      !svgElements.has(element.localName) ||
      element.namespaceURI !== 'http://www.w3.org/2000/svg'
    )
      throw new Error('SVG contains unsupported active or referenced content');
    for (const attr of element.attributes) {
      if (
        !svgAttributes.has(attr.name) ||
        (/url\s*\(|javascript:|data:|https?:/i.test(attr.value) && attr.name !== 'xmlns')
      )
        throw new Error('SVG attributes must be inert and self-contained');
      if (attr.name === 'xmlns' && attr.value !== 'http://www.w3.org/2000/svg')
        throw new Error('Unsupported SVG namespace');
    }
  }
  return new XMLSerializer().serializeToString(document.documentElement);
}

const animationUnavailable =
  'Animated profile-picture processing is not available yet. This image was not converted or saved.';

// This legacy still-image path must never flatten an animation while the bundled
// streaming JXL workflow is being integrated. Inspect framing before browser decode.
function requireStillRaster(bytes: Uint8Array, mime: string): void {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const text = new TextDecoder('ascii');
  let offset = mime === 'image/png' ? 8 : 12;
  while (offset < bytes.length) {
    const png = mime === 'image/png';
    const overhead = png ? 12 : 8;
    if (bytes.length - offset < overhead) throw new Error('Truncated image chunk');
    const length = view.getUint32(offset + (png ? 0 : 4), !png);
    const end = offset + overhead + length;
    if (end > bytes.length) throw new Error('Truncated image chunk');
    const tag = text.decode(bytes.subarray(offset + (png ? 4 : 0), offset + (png ? 8 : 4)));
    if (
      (png && tag === 'acTL') ||
      (!png &&
        (tag === 'ANIM' ||
          tag === 'ANMF' ||
          (tag === 'VP8X' && length > 0 && bytes[offset + 8] & 2)))
    )
      throw new Error(animationUnavailable);
    if (png && tag === 'IEND') return;
    offset = end + (!png && length % 2 ? 1 : 0);
  }
}

export async function importImage(bytes: Uint8Array): Promise<string> {
  if (!bytes.length || bytes.length > 5 * 1024 * 1024)
    throw new Error('Choose an image up to 5 MiB');
  let mime: string;
  let payload = bytes;
  const signature = new TextDecoder('ascii').decode(bytes.subarray(0, 6));
  if (
    signature === 'GIF87a' ||
    signature === 'GIF89a' ||
    (bytes[0] === 255 && bytes[1] === 10) ||
    (bytes[0] === 0 &&
      bytes[1] === 0 &&
      bytes[2] === 0 &&
      bytes[3] === 12 &&
      new TextDecoder('ascii').decode(bytes.subarray(4, 8)) === 'JXL ')
  )
    throw new Error(animationUnavailable);
  if (bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71) mime = 'image/png';
  else if (bytes[0] === 255 && bytes[1] === 216 && bytes[2] === 255) mime = 'image/jpeg';
  else if (
    new TextDecoder().decode(bytes.slice(0, 4)) === 'RIFF' &&
    new TextDecoder().decode(bytes.slice(8, 12)) === 'WEBP'
  )
    mime = 'image/webp';
  else {
    payload = new TextEncoder().encode(
      safeSvg(new TextDecoder('utf-8', { fatal: true }).decode(bytes)),
    );
    mime = 'image/svg+xml';
  }
  if (mime === 'image/png' || mime === 'image/webp') requireStillRaster(bytes, mime);
  let binary = '';
  for (const byte of payload) binary += String.fromCharCode(byte);
  const image = new Image();
  image.src = `data:${mime};base64,${btoa(binary)}`;
  await image.decode().catch(() => {
    throw new Error('Image could not be decoded');
  });
  if (
    !image.naturalWidth ||
    !image.naturalHeight ||
    image.naturalWidth * image.naturalHeight > 32_000_000
  )
    throw new Error('Image dimensions exceed 32 megapixels');
  const scale = Math.min(1, 512 / Math.max(image.naturalWidth, image.naturalHeight));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
  const context = canvas.getContext('2d');
  if (!context) throw new Error('Image conversion is unavailable');
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  const data = canvas.toDataURL('image/png');
  if (data.length >= 900_000) throw new Error('Converted image is too large');
  return data;
}
