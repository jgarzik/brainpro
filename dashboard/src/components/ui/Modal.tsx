import { useEffect, useRef, useId, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { clsx } from "clsx";
import { X } from "lucide-react";

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  footer?: ReactNode;
  children: ReactNode;
  className?: string;
  size?: "sm" | "md" | "lg";
}

const sizeStyles = {
  sm: "max-w-sm",
  md: "max-w-lg",
  lg: "max-w-2xl",
};

export function Modal({
  open,
  onClose,
  title,
  footer,
  children,
  className,
  size = "md",
}: ModalProps) {
  const titleId = useId();
  const modalRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);

  // Close on escape and implement focus trap
  useEffect(() => {
    if (!open) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }

      // Focus trap - keep focus within modal
      if (e.key === "Tab" && modalRef.current) {
        const focusableElements = modalRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
        );
        const firstElement = focusableElements[0];
        const lastElement = focusableElements[focusableElements.length - 1];

        if (e.shiftKey) {
          // Shift+Tab - if on first element, go to last
          if (document.activeElement === firstElement) {
            e.preventDefault();
            lastElement?.focus();
          }
        } else {
          // Tab - if on last element, go to first
          if (document.activeElement === lastElement) {
            e.preventDefault();
            firstElement?.focus();
          }
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  // Prevent scroll when open and manage focus
  useEffect(() => {
    if (open) {
      // Store the currently focused element to restore later
      previousActiveElement.current = document.activeElement as HTMLElement;
      document.body.style.overflow = "hidden";

      // Focus the modal or first focusable element
      requestAnimationFrame(() => {
        if (modalRef.current) {
          const firstFocusable = modalRef.current.querySelector<HTMLElement>(
            'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
          );
          firstFocusable?.focus();
        }
      });
    } else {
      document.body.style.overflow = "";

      // Restore focus to previous element
      if (previousActiveElement.current) {
        previousActiveElement.current.focus();
      }
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [open]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Modal content */}
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={title ? titleId : undefined}
        className={clsx(
          "relative w-full rounded-lg bg-white shadow-xl dark:bg-gray-800",
          "animate-slide-in",
          sizeStyles[size],
          className,
        )}
      >
        {/* Header */}
        {title && (
          <div className="flex items-center justify-between border-b border-gray-200 px-6 py-4 dark:border-gray-700">
            <h2
              id={titleId}
              className="text-lg font-semibold text-gray-900 dark:text-gray-100"
            >
              {title}
            </h2>
            <button
              type="button"
              onClick={onClose}
              aria-label="Close"
              className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-200"
            >
              <X className="h-5 w-5" />
            </button>
          </div>
        )}

        {/* Body */}
        <div className="p-6">{children}</div>

        {/* Footer */}
        {footer && (
          <div className="flex items-center justify-end gap-3 border-t border-gray-200 px-6 py-4 dark:border-gray-700">
            {footer}
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
