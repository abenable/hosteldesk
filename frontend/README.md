# HostelDesk frontend

React + TypeScript + Tailwind UI for the HostelDesk desktop app, built with Vite and running inside the Tauri v2 webview. See the [repository root README](../README.md) for setup, running, and building the whole app.

- Bundler: Vite, using `@vitejs/plugin-react` (the Oxc-based plugin — not the SWC one)
- Lint: `npm run lint` (Oxlint, config in `.oxlintrc.json`)
- Types: `npm run build` runs `tsc -b` before `vite build`
- Styling: Tailwind CSS — see `DESIGN.md` for tokens and conventions
- Component library: `src/components/ui/` (see `DESIGN.md`'s Components section)
