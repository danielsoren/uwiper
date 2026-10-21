import type { IconName } from '../ui/Icon';
import type { CleanupGroup } from './types';

export const CLEANUP_GROUPS: Record<CleanupGroup, { label: string; color: string; icon: IconName }> = {
  windows: { label: 'Windows', color: '#0A84FF', icon: 'monitor' },
  browsers: { label: 'Browsers', color: '#64D2FF', icon: 'globe' },
  apps: { label: 'Apps', color: '#E5E5EA', icon: 'chat' },
  developer: { label: 'Developer', color: '#BF5AF2', icon: 'code' },
};
