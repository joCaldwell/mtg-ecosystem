import { Component, type ReactNode } from "react";
import { Button, EmptyState, Page } from "../ui/index.ts";

/** A failed render must leave an understandable recovery action. */
export class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(error: Error) { console.error(error); }
  render() {
    return this.state.failed ? <Page><EmptyState title="Something went wrong">
      Reload to try again. Your saved decks are kept on the server.
      <Button onClick={() => window.location.reload()}>Reload</Button>
    </EmptyState></Page> : this.props.children;
  }
}
