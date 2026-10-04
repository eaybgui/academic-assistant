# Setup

Everything you need to run Academic Assistant from source on Windows 11. To use the built app
you only need the sections on the AI provider and the Microsoft Entra registration.

## 1. Prerequisites

| Requirement | Notes |
|---|---|
| Rust (stable, `x86_64-pc-windows-msvc`) | Install with [rustup](https://rustup.rs). The repo pins the `stable` channel with `rustfmt` and `clippy` in `rust-toolchain.toml`. |
| Visual Studio Build Tools | Install the **Desktop development with C++** workload (MSVC and a Windows SDK). Required to compile Rust on Windows and the bundled SQLite. |
| Node.js 22+ and npm | The frontend uses Vite 8 and Vitest 5. Check with `node -v` and `npm -v`. |
| WebView2 runtime | Ships with Windows 11. Tauri uses it to render the UI. |
| Ollama (optional) | Only if you want local analysis. Install from [ollama.com](https://ollama.com), then run `ollama pull qwen3:8b` (the default model; configurable in Settings). |
| jev API key (optional) | Only if you want the external provider. You enter the key in **Settings**; it is stored in Windows Credential Manager. Never put it in a file, an environment variable checked into the repo, or a chat. |
| Microsoft Entra app registration | Only for Microsoft 365 accounts. See the next section. |

If `cargo` is not on your `PATH` after installing Rust, add `%USERPROFILE%\.cargo\bin`:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

## 2. Microsoft Entra app registration

Microsoft 365 accounts sign in through Microsoft Graph with OAuth 2 (authorization code with
PKCE). The app is a public client, so there is **no client secret**. You register the app once
and enter its client ID in Settings.

1. Sign in to the [Microsoft Entra admin center](https://entra.microsoft.com) (or the Azure
   portal) and go to **Identity > Applications > App registrations** (Entra ID > App
   registrations), then choose **New registration**.
2. Name: anything, for example `Academic Assistant`.
3. Supported account types: **Accounts in any organizational directory (Any Microsoft Entra ID
   tenant - Multitenant)**. This lets you sign in with university tenants other than the one
   that owns the registration.
4. Redirect URI: choose the platform **Mobile and desktop applications** (public client) and
   enter `http://localhost`. Entra accepts any port on `localhost` for this platform; the app
   picks a random free port at sign-in time.
5. Select **Register**. On the overview page, copy the **Application (client) ID**.
6. Go to **API permissions > Add a permission > Microsoft Graph > Delegated permissions** and
   add `Mail.Read`, `User.Read` and `offline_access`. (`User.Read` is usually present already.)
7. Go to **Authentication** and set **Allow public client flows** to **Yes**. Save.
8. Do **not** create a client secret or certificate.
9. In the app, open **Settings**, paste the Application (client) ID into the Microsoft client
   ID field and save. Then add the account under **Accounts** (Microsoft 365) and press
   **Sign in with Microsoft**.

### University tenants and admin consent

Many universities restrict which third-party apps their users may consent to. If sign-in fails
with **"Your organization requires admin approval for this app"** (Microsoft error codes
`AADSTS65001` or `AADSTS90094`), your tenant administrator has to approve the app. Options:

- Ask the university IT department to grant admin consent for your app registration (give
  them the Application (client) ID and the three delegated permissions above).
- Sign in with a different account or tenant that permits user consent, if you have one.
- Forward the university mail to an account you can read over IMAP (or Microsoft 365 in a
  tenant you control) and add that account instead.

An account's tenant defaults to `organizations`. Reading mail only needs the read-only
`Mail.Read` permission; the app never sends, moves or deletes messages.

## 3. Run in development

```powershell
npm install
npm run tauri dev
```

`npm run tauri dev` starts the Vite dev server on port 1420 (`beforeDevCommand`), compiles the
Rust workspace and opens the app window. The first Rust build takes several minutes. To work on
the UI alone in a browser (without the Tauri backend, so commands will fail), `npm run dev`
starts only Vite.

## 4. Build

```powershell
npm run tauri build
```

This runs `npm run build` (TypeScript check and Vite build into `dist/`), compiles the Rust
workspace in release mode and produces the Windows installers (bundle targets are set to
`all` in `src-tauri/tauri.conf.json`). Output goes to `target/release/bundle/`, and the plain
executable to `target/release/`. Tauri may download the installer tooling (NSIS/WiX) the first
time.

For a faster check that the whole app compiles without producing installers:

```powershell
npm run build
npx tauri build --debug --no-bundle
```

## 5. Where data lives

The app stores its data in the Tauri application data directory, which on Windows is
`%APPDATA%\dev.eaybar.academic-assistant` (a folder named after the bundle identifier in
`src-tauri/tauri.conf.json`):

- `academic-assistant.db` (plus `-wal` and `-shm` files while the app runs): the SQLite
  database (accounts, catalog, settings, runs, results).
- `logs/academic-assistant.log.<YYYY-MM-DD>`: the daily rolling application log.

Passwords, refresh tokens and the jev API key are **not** in that folder; they are in Windows
Credential Manager under generic credentials whose names start with `academic-assistant/`.
See [app/logging-and-data.md](app/logging-and-data.md) for details. To reset the app, delete
the data folder and remove those credentials.

## 6. Next steps

- [user-guide.md](user-guide.md): using the app.
- [development.md](development.md): repo layout, quality gates and how to contribute.
- [testing.md](testing.md): running the test suites and the manual acceptance checklist.
