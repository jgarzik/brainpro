import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { clsx } from "clsx";

export type CardPadding = "sm" | "md" | "lg";

const paddingStyles: Record<CardPadding, string> = {
  sm: "p-3",
  md: "p-4",
  lg: "p-6",
};

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  title?: string;
  subtitle?: string;
  actions?: ReactNode;
  padding?: CardPadding;
}

export const Card = forwardRef<HTMLDivElement, CardProps>(
  (
    { className, title, subtitle, actions, padding = "md", children, ...props },
    ref,
  ) => {
    return (
      <div
        ref={ref}
        className={clsx(
          "rounded-lg border border-gray-200 bg-white shadow-sm",
          "dark:border-gray-700 dark:bg-gray-800",
          paddingStyles[padding],
          className,
        )}
        {...props}
      >
        {(title || subtitle || actions) && (
          <div className="mb-4 flex items-start justify-between">
            <div>
              {title && (
                <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
                  {title}
                </h3>
              )}
              {subtitle && (
                <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
                  {subtitle}
                </p>
              )}
            </div>
            {actions && (
              <div className="flex items-center gap-2">{actions}</div>
            )}
          </div>
        )}
        {children}
      </div>
    );
  },
);

Card.displayName = "Card";

/** Card header component for custom layouts */
export function CardHeader({
  className,
  children,
  ...props
}: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={clsx("mb-4 flex items-center justify-between", className)}
      {...props}
    >
      {children}
    </div>
  );
}

/** Card content component */
export function CardContent({
  className,
  children,
  ...props
}: HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={clsx("", className)} {...props}>
      {children}
    </div>
  );
}
