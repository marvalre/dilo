// Small SF-Symbol-like stroke icons (16×16, currentColor).
import type { ReactNode } from "react";

function Icon({ children, size = 16 }: { children: ReactNode; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.35}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {children}
    </svg>
  );
}

export const HomeIcon = () => (
  <Icon>
    <path d="M2.5 7.2 8 2.6l5.5 4.6" />
    <path d="M3.8 6.3v6.4c0 .4.3.8.8.8h2.3V10h2.2v3.5h2.3c.5 0 .8-.4.8-.8V6.3" />
  </Icon>
);

export const ClockIcon = () => (
  <Icon>
    <circle cx="8" cy="8" r="5.9" />
    <path d="M8 4.6V8l2.3 1.5" />
  </Icon>
);

export const BookIcon = () => (
  <Icon>
    <path d="M8 4.2C6.6 3.2 4.6 2.9 2.5 3.1v9.3c2.1-.2 4.1.1 5.5 1.1 1.4-1 3.4-1.3 5.5-1.1V3.1c-2.1-.2-4.1.1-5.5 1.1Z" />
    <path d="M8 4.2v9.3" />
  </Icon>
);

export const FaceIcon = () => (
  <Icon>
    <rect x="1.9" y="3.2" width="12.2" height="9.6" rx="4.8" />
    <path d="M6 7v.4M10 7v.4" strokeWidth={1.7} />
    <path d="M6.6 9.8h2.8" />
  </Icon>
);

export const GearIcon = () => (
  <Icon>
    <circle cx="8" cy="8" r="2.1" />
    <path d="M8 1.8v1.6M8 12.6v1.6M14.2 8h-1.6M3.4 8H1.8M12.4 3.6l-1.1 1.1M4.7 11.3l-1.1 1.1M12.4 12.4l-1.1-1.1M4.7 4.7 3.6 3.6" />
    <circle cx="8" cy="8" r="4.6" />
  </Icon>
);

export const SearchIcon = () => (
  <Icon size={14}>
    <circle cx="7" cy="7" r="4.4" />
    <path d="m10.3 10.3 3.2 3.2" />
  </Icon>
);

export const CopyIcon = () => (
  <Icon size={14}>
    <rect x="5.2" y="5.2" width="8.3" height="8.3" rx="1.8" />
    <path d="M10.8 5.2V4.2c0-1-.8-1.7-1.7-1.7H4.2c-1 0-1.7.8-1.7 1.7v4.9c0 1 .8 1.7 1.7 1.7h1" />
  </Icon>
);

export const PencilIcon = () => (
  <Icon size={14}>
    <path d="M10.6 2.9a1.6 1.6 0 0 1 2.3 2.3l-7.7 7.7-3 .7.7-3 7.7-7.7Z" />
    <path d="m9.5 4 2.3 2.3" />
  </Icon>
);

export const TrashIcon = () => (
  <Icon size={14}>
    <path d="M2.8 4.3h10.4M6.2 4.3V3c0-.4.3-.7.7-.7h2.2c.4 0 .7.3.7.7v1.3" />
    <path d="m3.9 4.3.6 8.4c.1.6.5 1 1.1 1h4.8c.6 0 1-.4 1.1-1l.6-8.4" />
  </Icon>
);

export const CheckIcon = ({ size = 14 }: { size?: number }) => (
  <Icon size={size}>
    <path d="m3.4 8.4 3 3 6.2-6.7" strokeWidth={1.7} />
  </Icon>
);

export const ArrowIcon = () => (
  <Icon size={14}>
    <path d="M2.8 8h10.4M9.3 4.1 13.2 8l-3.9 3.9" />
  </Icon>
);

export const ChevronDownIcon = () => (
  <Icon size={12}>
    <path d="m4 6 4 4 4-4" strokeWidth={1.6} />
  </Icon>
);

export const DownloadIcon = () => (
  <Icon size={14}>
    <path d="M8 2.5v8M4.6 7.3 8 10.7l3.4-3.4M2.8 13.3h10.4" />
  </Icon>
);

export const WarnIcon = () => (
  <Icon size={14}>
    <path d="M7.1 2.8a1 1 0 0 1 1.8 0l5.2 9.3c.4.7-.1 1.5-.9 1.5H2.8c-.8 0-1.3-.8-.9-1.5l5.2-9.3Z" />
    <path d="M8 6.3v3M8 11.3v.1" strokeWidth={1.6} />
  </Icon>
);
