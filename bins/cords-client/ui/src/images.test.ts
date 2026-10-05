// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { safeSvg } from './images';
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
