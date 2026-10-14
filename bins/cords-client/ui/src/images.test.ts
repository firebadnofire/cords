// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { importImage, safeSvg } from './images';

describe('untrusted SVG import boundary', () => {
  const svg = (body: string) =>
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">${body}</svg>`;
  it('accepts self-contained shapes', () => {
    expect(safeSvg(svg('<rect width="64" height="64" fill="#abc"/>'))).toContain('rect');
  });
  it.each([
    '<script>alert(1)</script>',
    '<foreignObject><div>HTML</div></foreignObject>',
    '<image href="https://tracker.example/pixel"/>',
    '<use href="#x"/>',
    '<rect onload="alert(1)"/>',
    '<rect fill="url(https://tracker.example/a)"/>',
    '<style>svg{background:url(https://tracker.example)}</style>',
    '<rect style="fill:red"/>',
    '<g xml:base="https://tracker.example"/>',
    '<animate attributeName="href" values="https://tracker.example"/>',
  ])('rejects active or externally referenced SVG: %s', (body) => {
    expect(() => safeSvg(svg(body))).toThrow();
  });
  it('rejects document entities and invalid XML', () => {
    expect(() =>
      safeSvg('<!DOCTYPE svg [<!ENTITY x SYSTEM "file:///secret">]>' + svg('&x;')),
    ).toThrow();
    expect(() => safeSvg('<svg>')).toThrow();
  });
});

afterEach(() => vi.unstubAllGlobals());

describe('static importer does not silently flatten animations', () => {
  function forbidDecode() {
    const image = vi.fn(() => {
      throw new Error('browser decode reached');
    });
    vi.stubGlobal('Image', image);
    return image;
  }

  it('rejects APNG before creating an Image or canvas', async () => {
    const decode = forbidDecode();
    // PNG signature and acTL with four frames and infinite playback.
    const bytes = new Uint8Array([
      137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 8, 97, 99, 84, 76, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0,
      0,
    ]);
    await expect(importImage(bytes)).rejects.toThrow('not converted or saved');
    expect(decode).not.toHaveBeenCalled();
  });

  it.each(['ANIM', 'ANMF', 'VP8X'])(
    'rejects animated WebP %s controls before browser decode',
    async (tag) => {
      const decode = forbidDecode();
      const bytes = new Uint8Array(24);
      bytes.set(new TextEncoder().encode('RIFF'), 0);
      bytes.set(new TextEncoder().encode('WEBP'), 8);
      bytes.set(new TextEncoder().encode(tag), 12);
      new DataView(bytes.buffer).setUint32(16, 4, true);
      bytes[20] = 2;
      await expect(importImage(bytes)).rejects.toThrow('not converted or saved');
      expect(decode).not.toHaveBeenCalled();
    },
  );

  it('gives an explicit unavailable state for GIF/JXL instead of trying SVG or a still conversion', async () => {
    const decode = forbidDecode();
    for (const bytes of [
      new TextEncoder().encode('GIF89a'),
      new Uint8Array([255, 10]),
      new Uint8Array([0, 0, 0, 12, 74, 88, 76, 32]),
    ]) {
      await expect(importImage(bytes)).rejects.toThrow('not converted or saved');
    }
    expect(decode).not.toHaveBeenCalled();
  });

  it('rejects truncated chunks before browser decode', async () => {
    const decode = forbidDecode();
    const bytes = new Uint8Array([
      137, 80, 78, 71, 13, 10, 26, 10, 255, 255, 255, 255, 97, 99, 84, 76, 0, 0, 0, 0,
    ]);
    await expect(importImage(bytes)).rejects.toThrow('Truncated image chunk');
    expect(decode).not.toHaveBeenCalled();
  });
});
