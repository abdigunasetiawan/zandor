import { type ReactNode, useId } from "react";
import { cx } from "../cx";

export type FieldControlProps = {
  id: string;
  "aria-describedby"?: string;
  invalid: boolean;
};

export type FieldProps = {
  label: string;
  description?: string;
  /** Pesan galat; bila ada, kontrol ditandai tidak valid. */
  error?: string;
  className?: string;
  /** Render-prop: teruskan props yang diberikan ke kontrol (Input, Textarea, Select, ...). */
  children: (control: FieldControlProps) => ReactNode;
};

/** Label + deskripsi + galat yang terhubung ke kontrol lewat id/aria-describedby. */
export function Field({ label, description, error, className, children }: FieldProps) {
  const id = useId();
  const descId = description ? `${id}-desc` : undefined;
  const errId = error ? `${id}-err` : undefined;
  const describedBy = [descId, errId].filter(Boolean).join(" ") || undefined;
  return (
    <div className={cx("z-field", className)}>
      <label htmlFor={id} className="z-label">
        {label}
      </label>
      {children({ id, "aria-describedby": describedBy, invalid: Boolean(error) })}
      {description && (
        <p id={descId} className="z-small">
          {description}
        </p>
      )}
      {error && (
        <p id={errId} role="alert" className="z-small z-field__error">
          {error}
        </p>
      )}
    </div>
  );
}
