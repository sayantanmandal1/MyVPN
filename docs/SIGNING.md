# Code signing (Windows)

By default the release workflow produces an **unsigned** installer, which works
but shows a SmartScreen / "unknown publisher" warning on first run. Signing the
installer with an Authenticode certificate removes that warning and is strongly
recommended for any public release.

Signing is **fully optional and opt-in**: the build succeeds with or without a
certificate. When the signing secrets below are present, the release workflow
imports the certificate and the Tauri bundler signs the installer automatically.

## What you need

An Authenticode code-signing certificate as a password-protected `.pfx` file.
You can obtain one from a public CA (e.g. DigiCert, Sectigo, SSL.com). An
**EV** or **OV** certificate gives the best SmartScreen reputation.

> Standard (non-EV) certificates still build reputation over time; EV
> certificates are trusted immediately but require a hardware token or a
> cloud-HSM signing flow (see "Cloud / HSM signing" below).

## Configure repository secrets

In the GitHub repository, go to **Settings → Secrets and variables → Actions**
and add:

| Secret | Value |
| --- | --- |
| `WINDOWS_CERTIFICATE` | Base64 of your `.pfx` file |
| `WINDOWS_CERTIFICATE_PASSWORD` | The `.pfx` password |

Produce the base64 value locally (do **not** commit the `.pfx`):

```powershell
[System.Convert]::ToBase64String([System.IO.File]::ReadAllBytes("certificate.pfx")) | Set-Clipboard
```

That's it. The next `git tag vX.Y.Z && git push --tags` will publish a signed
installer. The relevant config (`digestAlgorithm`, `timestampUrl`) already lives
in [app/src-tauri/tauri.conf.json](../app/src-tauri/tauri.conf.json); the
workflow injects the certificate thumbprint at build time.

## Local signed build (optional)

Install the certificate into `Cert:\CurrentUser\My`, then set the thumbprint in
`app/src-tauri/tauri.conf.json` under `bundle.windows`:

```json
"windows": {
  "certificateThumbprint": "YOUR_CERT_THUMBPRINT",
  "digestAlgorithm": "sha256",
  "timestampUrl": "http://timestamp.digicert.com"
}
```

Then build:

```powershell
cd app
npm run tauri build
```

> Never commit a real `certificateThumbprint` or the `.pfx`. Keep signing
> material in secrets / a local secret store only.

## Cloud / HSM signing (EV certificates)

EV certificates and modern CA requirements often mandate that the private key
never leaves an HSM. For those, replace `certificateThumbprint` with a
`signCommand` that calls your provider's signing tool, for example Azure Trusted
Signing or `signtool` with a cloud KSP:

```json
"windows": {
  "signCommand": "trusted-signing-cli -e %1"
}
```

`%1` is replaced with the path of the file to sign. Wire the provider's
credentials in as additional repository secrets and export them as environment
variables in the workflow before the build step.
