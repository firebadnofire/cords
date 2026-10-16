import { AvatarMemoryBudget } from './avatarPolicy';
export const avatarMemory = new AvatarMemoryBudget();
// The shared native-presentation cache bounds Base64 strings to 32 MiB. Keep its
// reservation while a profile editor may still hold a reviewed presentation.
const presentation = {};
avatarMemory.reserve(presentation, 32 * 1024 * 1024);
