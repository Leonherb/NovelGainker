import { Component, type ReactNode } from "react";
import { ZH_CN } from "../i18n/zh-CN";

type AppErrorBoundaryProps = {
  children: ReactNode;
  onReload?: () => void;
};

type AppErrorBoundaryState = {
  failed: boolean;
};

export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): AppErrorBoundaryState {
    return { failed: true };
  }

  private readonly reload = () => {
    if (this.props.onReload) {
      this.props.onReload();
      return;
    }

    window.location.reload();
  };

  render() {
    if (!this.state.failed) {
      return this.props.children;
    }

    return (
      <main className="error-boundary" role="alert">
        <section className="error-boundary-card">
          <p className="section-kicker">{ZH_CN.errorBoundary.eyebrow}</p>
          <h1>{ZH_CN.errorBoundary.title}</h1>
          <p>{ZH_CN.errorBoundary.message}</p>
          <p>{ZH_CN.errorBoundary.recovery}</p>
          <code>
            {ZH_CN.errorBoundary.codeLabel}：{ZH_CN.errorBoundary.code}
          </code>
          <button onClick={this.reload} type="button">
            {ZH_CN.errorBoundary.reload}
          </button>
        </section>
      </main>
    );
  }
}
