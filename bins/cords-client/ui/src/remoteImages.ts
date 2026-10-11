import { invoke } from '@tauri-apps/api/core';
import { importImage } from './images';

// Share bounded rasterization work across messages from the same author.
const images = new Map<string, Promise<string>>();
export function clearRemoteImages() {
  images.clear();
}
export function remoteImage(url: string): Promise<string> {
  const existing = images.get(url);
  if (existing) return existing;
  if (images.size >= 16) images.delete(images.keys().next().value!);
  const request = invoke<string>('load_image_url', { url }).then((encoded) =>
    importImage(Uint8Array.from(atob(encoded), (character) => character.charCodeAt(0))),
  );
  images.set(url, request);
  request.catch(() => {
    if (images.get(url) === request) images.delete(url);
  });
  return request;
}
