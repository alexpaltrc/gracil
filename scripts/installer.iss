; Gracil's installer, for one user and without an administrator: it puts a released Gracil in
; %LOCALAPPDATA%\Programs\Gracil, with a shortcut in the Start menu. Gracil keeps its data beside
; gracil.exe (ADR 0054), in the data folder, which updating or uninstalling never touches.
; Built by scripts/installer.ps1, which hands it the released folder and its version.

#ifndef Source
  #error Build it with scripts/installer.ps1
#endif

[Setup]
AppId={{C6E02D7F-511C-4C28-84E2-8B260225EE79}
AppName=Gracil
AppVersion={#Version}
AppVerName=Gracil {#Version}
AppPublisher=Alex Palacios
AppPublisherURL=https://github.com/alexpaltrc/gracil
DefaultDirName={localappdata}\Programs\Gracil
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
DisableWelcomePage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.22000
WizardStyle=modern
SetupIconFile=..\crates\gracil-ui\icons\gracil-app-dark.ico
UninstallDisplayIcon={app}\gracil.exe
UninstallDisplayName=Gracil
OutputDir={#Output}
OutputBaseFilename=Gracil-{#Version}-setup
Compression=lzma2/max
SolidCompression=yes
CloseApplications=force
RestartApplications=no

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[Files]
Source: "{#Source}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs

[InstallDelete]
; A newer Gracil may ship fewer files: what came with the last one goes, the data stays.
Type: filesandordirs; Name: "{app}\ffmpeg"
Type: filesandordirs; Name: "{app}\licenses"

[Icons]
Name: "{userprograms}\Gracil"; Filename: "{app}\gracil.exe"

[Run]
Filename: "{app}\gracil.exe"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; What Gracil itself put in Windows (ADR 0054): Send to's shortcut. Starting with Windows is a
; RunOnce value below; the main menu, written by an administrator, Gracil takes out itself.
Type: files; Name: "{usersendto}\Gracil.lnk"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\RunOnce"; ValueType: none; ValueName: "Gracil"; Flags: uninsdeletevalue dontcreatekey
