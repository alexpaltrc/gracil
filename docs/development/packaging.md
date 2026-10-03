# Packaging Gracil

Gracil ships as an MSIX package ([ADR 0005](../decisions/0005-msix-packaging.md),
[ADR 0023](../decisions/0023-package-build.md)). Everything goes through `scripts/package.ps1`.

## Build the layout

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1
```

This makes a release build and puts the package's files in `packaging/msix/out/layout`, with the
manifest filled in from `packaging/msix/identity.psd1` and the version from `Cargo.toml`.

## Try it on this computer

Registering the layout needs Windows' **Developer Mode** (Settings → System → For developers).

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1 -Register
```

Gracil then appears in the Start menu, and **Gracil** in File Explorer's right-click menu for
videos. Run it again after each change; it replaces the registered version. To remove it:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1 -Unregister
```

If File Explorer still shows an old menu, restart it (Task Manager → Windows Explorer → Restart).

## Make an installable package

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1 -Pack -Certificate C:\path\to\gracil.pfx -Password <password>
```

This writes `packaging/msix/out/Gracil_<version>_x64.msix`, signed. Without `-Certificate` the
package is created but not signed, which is only useful to check that it builds. The Windows SDK
(MakeAppx, MakePri and SignTool) must be installed.

The certificate's subject must match `Publisher` in `identity.psd1` exactly. A computer installs
the package only if it trusts that certificate; for a self-made test certificate, that means
adding it to the computer's *Trusted People*, which is the owner's decision. `.pfx` files are
never committed.

## Changing the identity

`packaging/msix/identity.psd1` holds the package name, publisher and publisher display name, and
nothing else repeats them. For the Microsoft Store, copy the three values from Partner Center
(the app's Product identity page). For direct distribution, use the subject of the code-signing
certificate as `Publisher`. Changing the identity makes Windows see a different app: uninstall
the old one first.

## Versions

The script maps `X.Y.Z` and `X.Y.Z-dev` to `X.Y.Z.0`
([versioning](versioning.md#msix-version-mapping)). Never edit the version in the manifest.

## Icons

`scripts/icons.ps1` draws the app-list icons in `packaging/msix/Assets` from the marble's shapes
and the Piedra palette: a graphite tile for a dark taskbar, an ivory one for a light taskbar
([ADR 0026](../decisions/0026-icon-tile.md)). The PNGs are committed; run the script again only
when the artwork or the palette changes. `package.ps1` indexes them in `resources.pri` with
MakePri (Windows SDK), so Windows can pick the right one; without that index it would show the
plain icon on the accent colour. Windows keeps icons in a cache, so a changed icon may only show
after signing out.
