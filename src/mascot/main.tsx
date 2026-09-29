import { Component, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { Mascot } from "./Mascot";
import "./mascot.css";

class Boundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(error: Error) {
    console.error(error);
    window.setTimeout(() => this.setState({ failed: false }), 1000);
  }
  render() {
    return this.state.failed ? null : this.props.children;
  }
}

createRoot(document.getElementById("root")!).render(
  <Boundary>
    <Mascot />
  </Boundary>,
);
