import type { CleanupGroup, CleanupItem } from '../../lib/types';

const ORDER: CleanupGroup[] = ['windows', 'browsers', 'apps', 'developer'];

/** Items grouped in display order, biggest first within a group. Empty groups are dropped. */
export function cleanupGroupsOf(items: CleanupItem[]) {
  return ORDER.map((group) => {
    const list = items.filter((i) => i.group === group).sort((a, b) => b.sizeBytes - a.sizeBytes);
    return { group, items: list, bytes: list.reduce((s, i) => s + i.sizeBytes, 0) };
  }).filter((g) => g.items.length > 0);
}
