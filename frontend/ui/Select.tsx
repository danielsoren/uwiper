import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../lib/cn';
import { Icon } from './Icon';

interface Props<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  label: string;
  disabled?: boolean;
  title?: string;
  className?: string;
}

/** A compact dropdown. Its menu opens on a layer above the page, so a card that clips its content can't cut it off. */
export function Select<T extends string>({ value, options, onChange, label, disabled, title, className }: Props<T>) {
  const [open, setOpen] = useState(false);
  const [place, setPlace] = useState<{ top?: number; bottom?: number; right: number } | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (!open || !button.current) return;
    const r = button.current.getBoundingClientRect();
    const right = window.innerWidth - r.right;
    // Opens upwards when there's no room below.
    setPlace(r.bottom + 8 + options.length * 32 > window.innerHeight ? { bottom: window.innerHeight - r.top + 4, right } : { top: r.bottom + 4, right });
  }, [open, options.length]);

  useEffect(() => {
    if (!open) return;
    menu.current?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
    const outside = (e: Event) => !menu.current?.contains(e.target as Node) && !button.current?.contains(e.target as Node);
    const onDown = (e: Event) => outside(e) && setOpen(false);
    const onScroll = (e: Event) => !menu.current?.contains(e.target as Node) && setOpen(false);
    const onResize = () => setOpen(false);
    window.addEventListener('mousedown', onDown);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onResize);
    return () => {
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onResize);
    };
  }, [open]);

  const close = () => {
    setOpen(false);
    button.current?.focus();
  };

  const onKey = (e: KeyboardEvent) => {
    const items = [...(menu.current?.querySelectorAll<HTMLElement>('[role="menuitemradio"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape' || e.key === 'Tab') close();
    else if (e.key === 'ArrowDown') items[(at + 1) % items.length]?.focus();
    else if (e.key === 'ArrowUp') items[(at - 1 + items.length) % items.length]?.focus();
    else return;
    e.preventDefault();
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={label}
        title={title}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        className={cn(
          'inline-flex h-[30px] items-center justify-between gap-1.5 rounded-lg border border-white/10 bg-fill-2 pl-2.5 pr-2 text-meta font-medium leading-[normal] enabled:hover:brightness-115 disabled:opacity-45',
          className,
        )}
      >
        {options.find((o) => o.value === value)?.label}
        <Icon name="chevron" size={12} className={cn('flex-none text-fg-3 transition-transform', open ? '-rotate-90' : 'rotate-90')} />
      </button>
      {open &&
        place &&
        createPortal(
          <div
            ref={menu}
            role="menu"
            aria-label={label}
            onKeyDown={onKey}
            style={place}
            className="fade-in fixed z-50 flex min-w-[184px] flex-col rounded-xl border border-white/10 bg-[#2a2a2e] p-1 shadow-[0_16px_44px_rgba(0,0,0,.55)]"
          >
            {options.map((o) => (
              <button
                key={o.value}
                type="button"
                role="menuitemradio"
                aria-checked={o.value === value}
                onClick={() => {
                  close();
                  if (o.value !== value) onChange(o.value);
                }}
                className="flex h-8 items-center gap-2 rounded-lg pl-2 pr-3 text-left text-meta leading-[normal] outline-none hover:bg-white/8 focus-visible:bg-white/8"
              >
                <Icon name="check" size={14} className={cn('flex-none text-accent-soft', o.value !== value && 'invisible')} />
                {o.label}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}
