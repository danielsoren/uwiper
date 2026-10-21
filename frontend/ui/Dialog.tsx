import { useEffect, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../lib/cn';

/**
 * A modal sheet. Without `onClose` it can't be dismissed (Escape or a click
 * outside), which keeps running steps from being cut short.
 */
export function Dialog({
  label,
  onClose,
  children,
  footer,
  wide,
}: {
  label: string;
  onClose?: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}) {
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  return createPortal(
    <div
      className="fade-in fixed inset-0 z-50 grid place-items-center bg-black/50 p-6"
      onMouseDown={(e) => e.target === e.currentTarget && onClose?.()}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={label}
        className={cn(
          'dialog-in flex max-h-full w-full flex-col overflow-hidden rounded-[18px] border border-white/10 bg-[#222226] shadow-[0_30px_90px_rgba(0,0,0,.6)]',
          wide ? 'max-w-[760px]' : 'max-w-[520px]',
        )}
      >
        <div className="scroll-thin min-h-0 flex-1 overflow-auto px-6 pb-5 pt-6">{children}</div>
        {footer && <div className="flex flex-wrap items-center justify-end gap-2.5 border-t border-line bg-white/2 px-6 py-4">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}
