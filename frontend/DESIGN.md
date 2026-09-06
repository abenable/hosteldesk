---
version: 1.0.0
name: HostelDesk-design-system
description: A clean, high-contrast desktop UI for hostel front-desk operations, built on a white canvas with Airbnb's Rausch (#ff385c) as the single accent color for primary actions and active navigation state. Type runs Inter at modest weights. Buttons and inputs are fully pill-shaped (`{rounded.full}`), cards use a 14px radius (`{rounded.md}`), and the app has no dark mode.

colors:
  primary: "#ff385c"
  primary-active: "#e00b41"
  primary-disabled: "#ffd1da"
  primary-error-text: "#c13515"
  luxe: "#460479"
  plus: "#92174d"
  ink: "#222222"
  body: "#3f3f3f"
  muted: "#6a6a6a"
  muted-soft: "#929292"
  hairline: "#dddddd"
  hairline-soft: "#ebebeb"
  border-strong: "#c1c1c1"
  canvas: "#ffffff"
  surface-soft: "#f7f7f7"
  surface-strong: "#f2f2f2"
  on-primary: "#ffffff"
  legal-link: "#428bff"
  scrim: "#000000"

typography:
  display-xl:
    fontFamily: "Inter, -apple-system, system-ui, Roboto, 'Helvetica Neue', sans-serif"
    fontSize: 28px
    fontWeight: 700
    lineHeight: 1.43
    letterSpacing: 0
  display-lg:
    fontFamily: "Inter, sans-serif"
    fontSize: 22px
    fontWeight: 500
    lineHeight: 1.18
    letterSpacing: -0.44px
  display-md:
    fontFamily: "Inter, sans-serif"
    fontSize: 21px
    fontWeight: 700
    lineHeight: 1.43
    letterSpacing: 0
  display-sm:
    fontFamily: "Inter, sans-serif"
    fontSize: 20px
    fontWeight: 600
    lineHeight: 1.20
    letterSpacing: -0.18px
  title-md:
    fontFamily: "Inter, sans-serif"
    fontSize: 16px
    fontWeight: 600
    lineHeight: 1.25
    letterSpacing: 0
  title-sm:
    fontFamily: "Inter, sans-serif"
    fontSize: 16px
    fontWeight: 500
    lineHeight: 1.25
    letterSpacing: 0
  body-md:
    fontFamily: "Inter, sans-serif"
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: 0
  body-sm:
    fontFamily: "Inter, sans-serif"
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.43
    letterSpacing: 0
  caption:
    fontFamily: "Inter, sans-serif"
    fontSize: 14px
    fontWeight: 500
    lineHeight: 1.29
    letterSpacing: 0
  caption-sm:
    fontFamily: "Inter, sans-serif"
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.23
    letterSpacing: 0
  badge:
    fontFamily: "Inter, sans-serif"
    fontSize: 11px
    fontWeight: 600
    lineHeight: 1.18
    letterSpacing: 0
  micro-label:
    fontFamily: "Inter, sans-serif"
    fontSize: 12px
    fontWeight: 700
    lineHeight: 1.33
    letterSpacing: 0
  uppercase-tag:
    fontFamily: "Inter, sans-serif"
    fontSize: 8px
    fontWeight: 700
    lineHeight: 1.25
    letterSpacing: 0.32px
    textTransform: uppercase
  button-md:
    fontFamily: "Inter, sans-serif"
    fontSize: 16px
    fontWeight: 500
    lineHeight: 1.25
    letterSpacing: 0
  button-sm:
    fontFamily: "Inter, sans-serif"
    fontSize: 14px
    fontWeight: 500
    lineHeight: 1.29
    letterSpacing: 0
  nav-link:
    fontFamily: "Inter, sans-serif"
    fontSize: 16px
    fontWeight: 600
    lineHeight: 1.25
    letterSpacing: 0

rounded:
  none: 0px
  xs: 4px
  sm: 8px
  md: 14px
  lg: 20px
  xl: 32px
  full: 9999px

spacing:
  xxs: 2px
  xs: 4px
  sm: 8px
  md: 12px
  base: 16px
  lg: 24px
  xl: 32px
  xxl: 48px
  section: 64px

components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.button-sm}"
    rounded: "{rounded.full}"
    height: 36px
  button-secondary:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    typography: "{typography.button-sm}"
    rounded: "{rounded.full}"
    height: 36px
    border: "1px solid {colors.ink}"
  button-tertiary:
    backgroundColor: transparent
    textColor: "{colors.ink}"
    typography: "{typography.button-sm}"
  badge:
    backgroundColor: "{colors.surface-strong}"
    textColor: "{colors.ink}"
    typography: "{typography.badge}"
    rounded: "{rounded.full}"
    padding: 2px 10px
  card:
    backgroundColor: "{colors.canvas}"
    rounded: "{rounded.md}"
    shadow: optional (`shadow-card` prop)
  stat-card:
    backgroundColor: "{colors.canvas}"
    rounded: "{rounded.md}"
    shadow: always
    padding: 20px
  table:
    backgroundColor: "{colors.canvas}"
    rounded: "{rounded.md}"
    border: "1px solid {colors.hairline}"
  input:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    typography: "{typography.body-md}"
  textarea:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    typography: "{typography.body-md}"
  select:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.full}"
    height: 36px
    border: "1px solid {colors.hairline}"
  search-input:
    backgroundColor: "{colors.canvas}"
    typography: "{typography.body-sm}"
  modal:
    backgroundColor: "{colors.canvas}"
    scrim: "{colors.scrim}"
  detail-drawer:
    backgroundColor: "{colors.canvas}"
    scrim: "{colors.scrim}"
  dropdown-item:
    backgroundColor: transparent
    textColor: "{colors.ink}"
    rounded: "{rounded.xs}"
  empty-state:
    backgroundColor: "{colors.canvas}"
    rounded: "{rounded.md}"
    padding: 40px
  stepper:
    activeColor: "{colors.ink}"
    completeColor: "{colors.ink}"
    inactiveColor: "{colors.hairline}"
  tabs:
    activeTextColor: "{colors.ink}"
    inactiveTextColor: "{colors.muted}"
  currency-amount:
    defaultColor: "{colors.ink}"
    mutedColor: "{colors.muted}"
    errorColor: "{colors.primary-error-text}"
  sidebar:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    accentColor: "{colors.surface-soft}"
    borderColor: "{colors.hairline}"
    width: 16rem
    widthCollapsedToIcon: 4rem
  avatar:
    rounded: "{rounded.full}"
    size: 32px
---

## Overview

HostelDesk is a Tauri desktop app for hostel front-desk staff — rooms, residents, check-ins, payments, and receipts. The base canvas is **pure white** (`{colors.canvas}`) with near-black ink (`{colors.ink}` — #222222) for text, and a single accent color, Airbnb's **Rausch** (`{colors.primary}` — #ff385c), inherited as the brand base and used for primary buttons and the active sidebar/nav item. There is no dark mode.

Type runs **Inter**, the same open-source family Airbnb's design guidance recommends as the closest substitute for its own custom font — HostelDesk ships Inter directly rather than treating it as a fallback. Display headlines sit at 20–28px in weight 600–700; body text is 14–16px at weight 400.

The shape language is **rounded**: buttons, the search input, and the `Select` trigger are fully pill-shaped (`{rounded.full}`, 36px tall), cards and tables use a 14px radius (`{rounded.md}`), and the sidebar/table borders use 1px hairlines (`{colors.hairline}`).

**Key Characteristics:**
- Single accent color: `{colors.primary}` carries primary buttons (`Button` variant `primary`/`pill`) and the active sidebar item. Most of the UI is white + ink with one accent moment per screen.
- Sidebar + header shell: a fixed 16rem sidebar (`components/ui/sidebar.tsx`, collapsible to a 4rem icon rail) plus a top `site-header`, used across every view (`Layout.tsx`).
- Dashboard `StatCard`s (`{component.stat-card}`) always carry the one shadow tier (`shadow-card`) to separate them from the flat page background.
- Data-heavy views (Residents, Invoices) lean on `Table` for lists and `DetailDrawer`/`Modal` for record detail and edit forms, rather than full page navigation.
- 8px base spacing system (`{spacing}` tokens), consistent with the inherited Airbnb scale.

## Colors

### Brand & Accent
- **Rausch** (`{colors.primary}` — #ff385c): The single accent. Used for primary/pill `Button` backgrounds and the active sidebar item's fill.
- **Rausch Active** (`{colors.primary-active}` — #e00b41): `active:` state on primary/pill buttons.
- **Rausch Disabled** (`{colors.primary-disabled}` — #ffd1da): `disabled:` state on primary/pill buttons.
- **Luxe** (`{colors.luxe}`) / **Plus** (`{colors.plus}`): Carried over from the inherited Airbnb palette; not currently used anywhere in HostelDesk's UI.

### Surface
- **Canvas** (`{colors.canvas}` — #ffffff): Page and card background. The app has no dark mode.
- **Surface Soft** (`{colors.surface-soft}` — #f7f7f7): Sidebar hover/accent background.
- **Surface Strong** (`{colors.surface-strong}` — #f2f2f2): `Badge` background.

### Hairlines & Borders
- **Hairline** (`{colors.hairline}` — #dddddd): Default 1px border — `Table` wrapper, `Select` trigger, sidebar border.
- **Hairline Soft** (`{colors.hairline-soft}` — #ebebeb): Lighter divider variant.
- **Border Strong** (`{colors.border-strong}` — #c1c1c1): Heavier stroke for emphasized outlines.

### Text
- **Ink** (`{colors.ink}` — #222222): Default text color — headings, body, sidebar labels, table cells. Never pure black.
- **Body** (`{colors.body}` — #3f3f3f): Secondary running text.
- **Muted** (`{colors.muted}` — #6a6a6a): Inactive sidebar labels, table sub-labels, hints.
- **Muted Soft** (`{colors.muted-soft}` — #929292): Disabled text.
- **On Primary** (`{colors.on-primary}` — #ffffff): Text on Rausch-filled buttons.

### Semantic
- **Error** (`{colors.primary-error-text}` — #c13515): `CurrencyAmount`'s `error` state (residents who owe money) and form validation errors on `Input`/`Textarea`/`Select`.
- **Legal Link Blue** (`{colors.legal-link}` — #428bff): Carried over from the Airbnb palette; not currently used in HostelDesk's UI.

### Scrim
- **Scrim** (`{colors.scrim}` — #000000 at partial opacity): Backdrop behind `Modal` and `DetailDrawer`.

## Typography

### Font Family
The app runs **Inter** everywhere, falling back to `-apple-system, system-ui, Roboto, "Helvetica Neue", sans-serif` (`tailwind.config.js`, `index.css`). There is a single font family — no separate display face.

### Hierarchy

| Token | Size | Weight | Use in HostelDesk |
|---|---|---|---|
| `{typography.display-xl}` | 28px | 700 | Largest page-level numbers (e.g. dashboard hero stat) |
| `{typography.display-lg}` | 22px | 500 | Drawer/modal titles |
| `{typography.display-md}` | 21px | 700 | Section headings |
| `{typography.display-sm}` | 20px | 600 | View page titles (Rooms, Residents, Invoices) |
| `{typography.title-md}` | 16px | 600 | `StatCard` labels, card headings |
| `{typography.title-sm}` | 16px | 500 | Settings section sub-headings |
| `{typography.body-md}` | 16px | 400 | Form field values, drawer body text |
| `{typography.body-sm}` | 14px | 400 | Table cell text, `Button` labels |
| `{typography.caption}` | 14px | 500 | `Input`/`Select` labels |
| `{typography.caption-sm}` | 13px | 400 | Field hints, error text |
| `{typography.badge}` | 11px | 600 | `Badge` text (status tags) |
| `{typography.micro-label}` | 12px | 700 | Small emphasized labels |
| `{typography.uppercase-tag}` | 8px | 700 | Uppercase micro-tags |
| `{typography.button-md}` | 16px | 500 | Larger button labels |
| `{typography.button-sm}` | 14px | 500 | Default `Button` label size |
| `{typography.nav-link}` | 16px | 600 | Sidebar nav item labels |

Two tokens from the inherited Airbnb scale — `rating-display` (64px) and `link` (14px/400 inline-link style) — were never carried into `tailwind.config.js` and are not used anywhere in the app; they are omitted above.

## Layout

- **Base unit:** 4px (`{spacing.xxs}` 2px · `{spacing.xs}` 4px · `{spacing.sm}` 8px · `{spacing.md}` 12px · `{spacing.base}` 16px · `{spacing.lg}` 24px · `{spacing.xl}` 32px · `{spacing.xxl}` 48px · `{spacing.section}` 64px).
- **Shell:** a fixed sidebar (`16rem`, collapsible to a `4rem` icon rail below the 768px breakpoint — see `hooks/use-mobile.ts`) plus a header, wrapping every view's content area.
- **Cards:** `{spacing.lg}` (24px) internal padding is typical for `Card`/`StatCard`; tables use `{spacing.sm}`–`{spacing.md}` cell padding.

## Elevation

The system has one shadow tier plus the flat baseline, defined once as `shadow-card` in `tailwind.config.js` (`boxShadow.card`):

```
rgba(0,0,0,0.02) 0 0 0 1px, rgba(0,0,0,0.04) 0 2px 6px 0, rgba(0,0,0,0.1) 0 4px 8px 0
```

- **Flat:** the page background, sidebar, and most cards.
- **Raised:** `StatCard` always applies `shadow-card`; plain `Card` applies it only when its `shadow` prop is set.

There are no other elevation tiers — depth comes from this one shadow plus hairline borders and rounded-corner clipping.

## Components

Real components live in `frontend/src/components/ui/`. This list gives each one's purpose and key visual treatment — see the source file for full props.

- **`Button`** (`Button.tsx`) — variants `primary`/`pill` (Rausch fill, `{rounded.full}`, white text), `secondary` (white fill, 1px ink border), `tertiary` (text-only, underline on hover). Sizes `md`/`sm`.
- **`Badge`** (`Badge.tsx`) — pill-shaped status tag: `{colors.surface-strong}` fill, ink text, `{typography.badge}`.
- **`Card`** (`Card.tsx`) — white surface, `{rounded.md}`, optional `shadow-card` via a `shadow` prop.
- **`StatCard`** (`StatCard.tsx`) — a `Card` with a shadow always applied, label/value/hint/trend layout; used on the Dashboard.
- **`CurrencyAmount`** (`CurrencyAmount.tsx`) — renders a formatted decimal amount; `muted` or `error` (unpaid balance) text color variants.
- **`Table`** (`Table.tsx`) — scrollable wrapper (`overflow-x-auto`) with a `{rounded.md}` border in `{colors.hairline}`.
- **`Input`** / **`Textarea`** (`Input.tsx`, `Textarea.tsx`) — labeled form fields with an `error` message slot; `Input` supports a password-reveal toggle.
- **`Select`** (`Select.tsx`) — Radix-based dropdown select styled as a `{rounded.full}` pill trigger with a hairline border, matching `Input`'s label/error pattern.
- **`SearchInput`** (`SearchInput.tsx`) — text input with a leading search icon.
- **`Modal`** (`Modal.tsx`) — Radix Dialog, `md`/`lg` width, title/description/footer slots.
- **`DetailDrawer`** (`DetailDrawer.tsx`) — Radix Dialog styled as a slide-in drawer for record detail views (e.g. a resident's profile).
- **`Dropdown`** (`Dropdown.tsx`) — Radix DropdownMenu items, with a `danger` variant for destructive actions.
- **`EmptyState`** (`EmptyState.tsx`) — centered icon/title/description/action inside a `Card`, shown for empty lists.
- **`Stepper`** (`Stepper.tsx`) — horizontal numbered step indicator (complete/active/upcoming), used in the check-in flow.
- **`Tabs`** (`Tabs.tsx`) — Radix Tabs, used to split Settings into sections.
- **`avatar`** (`avatar.tsx`) — Radix Avatar, 32px circular, used in the sidebar user menu and header.
- **`sidebar`** (`sidebar.tsx`) — the app shell's collapsible navigation rail (shadcn-derived), 16rem expanded / 4rem icon-only, switching to an off-canvas sheet under the 768px mobile breakpoint.
- **`icons`** (`icons.tsx`) — the app's hand-drawn 24×24 stroke icon set (Dashboard, Rooms, Residents, Invoice, Settings, nav chevrons, etc.).

## Responsive Behavior

HostelDesk is a fixed-window desktop app (Tauri webview), not a responsive marketing site — there is no tablet/mobile breakpoint ladder to document. The one real breakpoint is `hooks/use-mobile.ts`'s `MOBILE_BREAKPOINT` (768px), which the sidebar (`sidebar.tsx`) uses to switch from a fixed 16rem/4rem rail to an off-canvas sheet. No other component currently branches on viewport width.

## Known Gaps

- **Hover states:** individual component hover treatments are defined in Tailwind utility classes per-component rather than as documented tokens here.
- **Loading/skeleton states:** not currently implemented anywhere in the app.
- **Inherited-but-unused tokens:** `{colors.luxe}`, `{colors.plus}`, `{colors.legal-link}`, and the typography tokens `rating-display`/`link` came from the Airbnb palette this system was bootstrapped from and aren't used in HostelDesk today — kept here only so removing them from `tailwind.config.js` (if that ever happens) doesn't silently orphan this doc.
