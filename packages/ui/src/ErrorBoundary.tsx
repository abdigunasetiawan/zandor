// Error boundary per panel (PRD §11): galat render di satu bagian tidak memutihkan seluruh aplikasi.
import type { Logger } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Component, type ErrorInfo, type ReactNode } from "react";

type Props = {
  /** Nama bagian untuk log, mis. "panel.chat". */
  name: string;
  logger: Logger;
  children: ReactNode;
};
type State = { error: Error | null };

export class ErrorBoundary extends Component<Props, State> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    this.props.logger.error(`render gagal di ${this.props.name}`, { error, componentStack: info.componentStack ?? "" });
  }

  reset = () => this.setState({ error: null });

  override render() {
    if (this.state.error) return <ErrorFallback name={this.props.name} onRetry={this.reset} />;
    return this.props.children;
  }
}

function ErrorFallback({ name, onRetry }: { name: string; onRetry: () => void }) {
  const { t } = useI18n();
  return (
    <div role="alert" data-error-boundary={name} className="z-error-fallback">
      <p className="z-error-fallback__title">{t("error.panelTitle")}</p>
      <p className="z-error-fallback__body">{t("error.panelBody")}</p>
      <button type="button" onClick={onRetry}>
        {t("error.retry")}
      </button>
    </div>
  );
}
