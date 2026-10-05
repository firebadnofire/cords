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
export async function importImage(bytes: Uint8Array): Promise<string> {
  if (!bytes.length || bytes.length > 5 * 1024 * 1024)
    throw new Error('Choose an image up to 5 MiB');
  let mime: string;
  let payload = bytes;
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
