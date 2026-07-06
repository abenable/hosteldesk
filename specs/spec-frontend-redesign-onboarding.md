# Spec: Frontend Redesign + Hostel Onboarding Workflow

## Goal

Completely re-imagine the Hostel Desk frontend following the design system in
`frontend/DESIGN.md` (Airbnb-derived: white canvas, single Rausch accent
`#ff385c`, soft rounded shapes, Inter substitute for Airbnb Cereal VF, modest
type weights, photography/whitespace-led hierarchy). Ship a complete hostel
onboarding workflow that carries a new manager from "no hostel set up" to
"ready to check in their first resident."

This is a frontend-only change. Do **not** modify any Rust crate. New Tauri
commands the onboarding flow needs but does not have yet are listed under
"Backend gaps" — for those, the frontend must degrade gracefully (see rules).

---

## What exists today (read these first, reuse — do not rewrite the data layer)

- `frontend/src/lib/tauri.ts` — typed wrappers over `invoke`. Already exports:
  `Room`, `ResidentInput`, `AllocationInput`, `LicensePayload`,
  `getRooms(hostelId=1)`, `addResident`, `createAllocation`,
  `getLicenseStatus`, `activateLicense`, `generateTrialLicense`.
- `frontend/src/lib/toasts.ts` — `addToast(msg, kind)` + the `ToastContainer`.
  Reuse for all feedback. Do not invent a second toast system.
- `frontend/src/components/ToastContainer.tsx` — keep, restyle to the system.
- `frontend/src/components/LockScreen.tsx` — current license-activation gate.
  This becomes the **first** step of onboarding conceptually (no license → no
  app), but the onboarding wizard itself runs **after** unlock, when the app
  has a license but the hostel profile/rooms are not yet set up.
- `frontend/src/App.tsx` — top-level state machine: `checking →
  (no license → LockScreen) → (license → Layout+view)`. You will extend this
  state machine to add an onboarding branch.

### Tauri commands actually registered (`crates/app/src/lib.rs`)

```
get_rooms, add_resident, create_allocation, record_payment,
record_payment_and_generate_receipt, generate_and_open_receipt,
generate_trial_license, get_license_status, activate_license,
create_staff_user, login
```

`get_rooms` takes a `hostel_id` and currently the whole frontend hardcodes
`hostel_id = 1` (see `getRooms()` default and `CheckInForm`'s
`hostel_id: 1`). There is **no** command to create a hostel, create rooms, or
fetch the hostel profile. Treat those as gaps (below).

---

## Backend gaps (the frontend must NOT pretend these exist)

The following do not exist as Tauri commands today:

1. `create_hostel` — no way to persist hostel name/address/currency/logo.
2. `add_room` / `bulk_add_rooms` — no way to create rooms from the UI. The
   schema and `db` repo have no `add_room` either.
3. `get_hostel_profile` — no way to read whether onboarding is complete.
4. `create_staff_user` **does** exist — onboarding can use it for the
   "create your admin account" step. See `commands.rs:309`. Payload:
   `{ hostel_id, name, username, password, role }`.
5. `login` **does** exist — `commands.rs:337`.

**Graceful-degradation rule:** for steps 1–3 (which have no backend), the
onboarding UI must still collect the inputs and persist them to
`localStorage` under a `hosteldesk.onboarding` key, with a clear toast /
banner: "Saved locally — full persistence arrives with the next backend
release." This keeps the workflow demonstrable end-to-end without faking
`invoke` calls that will throw. Mark every such stub with a
`// ponytail: local-only until create_hostel/add_room commands land` comment.

Do **not** add fake `invoke` wrappers in `tauri.ts` for missing commands.
Local-only persistence lives in a small `frontend/src/lib/onboarding.ts`
module (localStorage get/set + a typed `HostelProfile` and `RoomDraft`).

---

## Design system implementation

Port `frontend/DESIGN.md` into the Tailwind config so tokens are real classes,
not inline styles scattered across components.

### `frontend/tailwind.config.js`

Extend `theme` with:

- `colors`: map every token from DESIGN.md `colors:` block to a Tailwind color
  alias — `primary` (`#ff385c`), `primary-active` (`#e00b41`),
  `primary-disabled` (`#ffd1da`), `luxe`, `plus`, `ink` (`#222`),
  `body` (`#3f3f3f`), `muted` (`#6a6a6a`), `muted-soft` (`#929292`),
  `hairline` (`#dddddd`), `hairline-soft` (`#ebebeb`),
  `border-strong` (`#c1c1c1`), `canvas` (`#ffffff`), `surface-soft` (`#f7f7f7`),
  `surface-strong` (`#f2f2f2`), `on-primary` (`#ffffff`),
  `error-text` (`#c13515`), `legal-link` (`#428bff`), `scrim` (`#000000`).
- `fontFamily.sans`: `'Inter', -apple-system, system-ui, Roboto, 'Helvetica Neue', sans-serif`.
  Load Inter from Google Fonts via `index.html` `<link>` (Cereal is not
  licensed; DESIGN.md §"Note on Font Substitutes" sanctions Inter).
- `borderRadius`: `xs` 4px, `sm` 8px, `md` 14px, `lg` 20px, `xl` 32px, `full`
  9999px (override Tailwind's defaults so `rounded-md` = 14px as DESIGN.md
  specifies).
- `fontSize`: map the typography scale tokens you actually use (display-xl 28/
  700, display-lg 22/500, display-md 21/700, display-sm 20/600, title-md 16/600,
  title-sm 16/500, body-md 16/400, body-sm 14/400, caption 14/500,
  caption-sm 13/400, badge 11/600, micro-label 12/700, uppercase-tag 8/700,
  button-md 16/500, button-sm 14/500, nav-link 16/600).
- `boxShadow`: a single `card` token =
  `rgba(0,0,0,0.02) 0 0 0 1px, rgba(0,0,0,0.04) 0 2px 6px 0, rgba(0,0,0,0.1) 0 4px 8px 0`
  (the one shadow tier from DESIGN.md §Elevation).
- `spacing`: extend with `section: 64px`, `xxl: 48px` if not already present.

### `frontend/src/index.css`

- Replace the `body` font stack with the Inter stack.
- Set `body { background: #fff; color: #222; }`.
- Add a `.surface-card` utility and a `.btn` base if helpful, but prefer tokens
  directly in components.

### Component primitives (`frontend/src/components/ui/`)

Create the minimum primitives the redesign needs. Each is a thin wrapper — no
over-abstraction, one file each:

- `Button.tsx` — variants: `primary` (Rausch, `rounded-sm`, h48), `secondary`
  (white + ink outline), `tertiary` (text), `pill` (Rausch pill). Sizes md/sm.
  Disabled → `primary-disabled`. Active press → `primary-active`.
- `Input.tsx` / `Select.tsx` / `Textarea.tsx` — `text-input` spec: white, 1px
  hairline, `rounded-sm`, h56, label above in caption/muted, focus → 2px ink
  border (no glow/ring).
- `Card.tsx` — white, `rounded-md`, optional `shadow-card`.
- `Badge.tsx` — `rounded-full` pill, badge type scale.
- `Stepper.tsx` — horizontal step indicator for the onboarding wizard (numbered
  circles + labels, ink underline on active, hairline on completed, muted on
  upcoming). Used only by the onboarding flow.

Do not build a generic design-system library. If a primitive is used in only
one place, inline it instead.

---

## Onboarding workflow (the headline deliverable)

A multi-step wizard shown when the user has a valid license but has not yet
completed initial setup. Persist completion in `localStorage`
(`hosteldesk.onboarding.complete = boolean`) so it only shows once; provide a
"Restart onboarding" action in Settings that clears the flag.

### Trigger condition (extend `App.tsx` state machine)

```
checking → (no license → LockScreen) → (license → check onboarding)
  → onboarding not complete → <OnboardingWizard/>
  → onboarding complete → <Layout/> + active view
```

Read completion state from `lib/onboarding.ts` (`isOnboardingComplete()`).
Re-evaluate on a custom event `hosteldesk:onboarding-complete` so the wizard
can hand off to the main app without a full reload.

### Steps

The wizard is a single `frontend/src/views/Onboarding.tsx` with internal step
state (no router — keep it lazy). Use the `Stepper` primitive at the top.

**Step 1 — Hostel profile.**
Fields: hostel name (required), address, phone, currency (select: USD, UGX,
KES, RWF, TZS, NGN — default by locale is fine), optional logo path (text
input for now; no file picker wiring needed). On next: validate name,
`saveHostelProfile()` to localStorage (ponytail stub). Show the local-only
banner described above.

**Step 2 — Room inventory.**
Let the manager add rooms quickly: a repeating row with room number, type
(single/double/dorm), capacity, floor, rate. "Add room" button appends a row;
rows are editable and deletable. Seed with 3 empty rows. Show a live count
"X rooms ready." On next: `saveRoomDrafts()` to localStorage (ponytail stub).
This is the data that a future `bulk_add_rooms` command will consume.

**Step 3 — Admin account.**
Create the first staff user via the **real** `create_staff_user` command
(`hostel_id: 1`, `role: "admin"`). Fields: name, username, password (with
confirm + minimum-length validation), role fixed to admin (display as
read-only). Use the typed wrapper — add `createStaffUser` to `tauri.ts`
(it's missing from the current file even though the command exists). On
success, advance. On error (e.g. username taken — the backend returns a
string), surface via toast and stay on the step.

**Step 4 — Review & finish.**
Show a read-only summary: hostel name, room count by type, admin username.
"Finish" button → set `hosteldesk.onboarding.complete = true`, emit
`hosteldesk:onboarding-complete`, which flips `App.tsx` into the main layout.
Land the user on the Dashboard.

### Back / forward rules

- Back is always allowed and preserves entered data (keep wizard state in
  React, only persist to localStorage on explicit Next/Finish).
- Forward is disabled until the step's required fields are valid.
- Step 3 (admin account) is the only step that calls a real backend command;
  if it fails, do not advance.

---

## Restyle the existing app shell + views

Bring these in line with the design system. Functionality stays the same; only
markup/classes change.

- `Layout.tsx` — replace the slate palette with canvas/ink/hairline. Sidebar
  becomes a white left rail with a Rausch wordmark, nav-link type, ink
  underline on active. Mobile header keeps the hamburger pattern.
- `Sidebar.tsx` — nav items: Dashboard, Rooms, Residents, Settings. Use
  `nav-link` type; active = ink + 2px ink underline; inactive = muted.
- `LockScreen.tsx` — full-bleed white, centered card, Rausch primary CTA
  ("Activate"), secondary "Start 30-day trial" calling `generateTrialLicense`
  then `activateLicense`. This is the user's first brand impression — make it
  calm, not busy.
- `RoomStatusBoard.tsx` — color-coded grid. Keep vacant/occupied/maintenance
  semantics but use surface-soft / ink / muted-soft fills with Rausch only for
  the "maintenance alert" dot, not the whole cell. Cards are `rounded-md`.
- `CheckInForm.tsx` — restyle with `Input`/`Select`/`Button` primitives.
  Keep the exact field set and submission logic (it calls real commands).
- `Dashboard.tsx` / `Rooms.tsx` / `Residents.tsx` / `Settings.tsx` — these are
  tiny stubs today (≤20 lines). Restyle their shells to the system. Settings
  must include the "Restart onboarding" action (clears
  `hosteldesk.onboarding.complete` and reloads to the wizard).
- `ToastContainer.tsx` — restyle toasts: white card, `rounded-md`, `shadow-card`,
  ink text, a 4px Rausch left border on success, ink on info, `error-text` on
  error.

---

## Files to create / modify

Create:
- `frontend/src/components/ui/{Button,Input,Select,Textarea,Card,Badge,Stepper}.tsx`
- `frontend/src/lib/onboarding.ts`
- `frontend/src/views/Onboarding.tsx`

Modify:
- `frontend/tailwind.config.js` (tokens)
- `frontend/src/index.css` (base)
- `frontend/index.html` (Inter link)
- `frontend/src/App.tsx` (state machine + onboarding branch)
- `frontend/src/lib/tauri.ts` (add `createStaffUser` + `login` typed wrappers,
  matching the existing `ResidentInput`/`AllocationInput` pattern; do not touch
  existing wrappers)
- `frontend/src/components/{Layout,Sidebar,LockScreen,RoomStatusBoard,
  CheckInForm,ToastContainer}.tsx`
- `frontend/src/views/{Dashboard,Rooms,Residents,Settings}.tsx`

Do not delete `RoomStatusBoard` or `CheckInForm` — they hold working logic.

---

## Verification (the implementer must run these before reporting done)

```sh
cd frontend
npm run lint      # oxlint — must pass
npm run build     # tsc -b && vite build — must pass with no TS errors
```

If a Tauri command is invoked, the typed wrapper must match the Rust payload
exactly (snake_case keys, correct types). Cross-check against
`crates/app/src/commands.rs`.

The implementer cannot run the Tauri shell here, so for runtime verification:
after `npm run build`, open `frontend/dist/index.html` logic is not
server-driven — but since the app calls `invoke`, a pure-browser load will
fail at the Tauri boundary. That's expected. Instead, verify by: (a) lint +
build passing, (b) confirming no `invoke` is called for the stubbed
onboarding steps (only `create_staff_user` is real), (c) confirming
`localStorage` read/write paths in `onboarding.ts` are symmetric.

## Out of scope

- Any Rust change.
- A router (react-router etc.) — keep the existing view-state pattern in
  `App.tsx`.
- New dependencies beyond what `package.json` already has. Use Inter via
  Google Fonts `<link>`, not a font package.
- Payments / receipts UI redesign — that's a later spec. Only the components
  listed above.
