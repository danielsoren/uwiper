import { clsx, type ClassValue } from 'clsx';
import { extendTailwindMerge } from 'tailwind-merge';

// Teach tailwind-merge the custom scales from app/index.css, otherwise it
// mistakes `text-micro` for a color and drops it next to `text-fg-3`.
const twMerge = extendTailwindMerge({
  extend: {
    theme: {
      text: ['micro', 'small', 'meta', 'body', 'label', 'title', 'h2', 'h1', 'stat', 'hero'],
      radius: ['control', 'tile', 'card'],
    },
  },
});

/** Joins class names; later Tailwind classes win over earlier ones. */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
