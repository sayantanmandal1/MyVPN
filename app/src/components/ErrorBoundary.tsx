import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Catches render-time errors anywhere in the UI tree and shows a recoverable
 * fallback instead of a blank window. The VPN engine runs in the Rust backend,
 * so an interface error never affects an active tunnel.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Surface in the devtools console for diagnostics.
    console.error("UI error boundary caught:", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-screen flex-col items-center justify-center gap-4 p-8 text-center">
          <div className="text-lg font-semibold">Something went wrong</div>
          <div className="max-w-md text-sm text-[color:var(--color-muted)]">
            The interface hit an unexpected error. Any active VPN connection is
            unaffected — reload to continue.
          </div>
          <button
            onClick={() => window.location.reload()}
            className="rounded-xl bg-accent px-4 py-2 text-sm font-medium text-black transition hover:opacity-90"
          >
            Reload
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
