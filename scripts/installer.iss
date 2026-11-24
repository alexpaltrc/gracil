; Gracil's installer, for one user and without an administrator: it puts Gracil in
; %LOCALAPPDATA%\Programs\Gracil, with a shortcut in the Start menu. Gracil keeps its data beside
; gracil.exe (ADR 0054), in the data folder, which updating or uninstalling never touches.
; Built by scripts/installer.ps1, which hands it the folder to install and its version. With
; /DDev it installs Gracil Dev instead, a separate app with its own name, folder and icon that
; never touches the daily Gracil (ADR 0082).

#ifndef Source
  #error Build it with scripts/installer.ps1
#endif

#ifdef Dev
  #define Name "Gracil Dev"
  #define Id "{{E8F0D6C8-6F2A-42BC-AAB1-692575314DE9}"
  #define Icon "gracil-dev-dark.ico"
#else
  #define Name "Gracil"
  #define Id "{{C6E02D7F-511C-4C28-84E2-8B260225EE79}"
  #define Icon "gracil-app-dark.ico"
#endif

[Setup]
AppId={#Id}
AppName={#Name}
AppVersion={#Version}
AppVerName={#Name} {#Version}
AppPublisher=Alex Palacios
AppPublisherURL=https://github.com/alexpaltrc/gracil
DefaultDirName={localappdata}\Programs\{#Name}
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
DisableWelcomePage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.22000
WizardStyle=modern
SetupIconFile=..\crates\gracil-ui\icons\{#Icon}
UninstallDisplayIcon={app}\gracil.exe
UninstallDisplayName={#Name}
OutputDir={#Output}
OutputBaseFilename={#Name}-{#Version}-setup
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
Name: "{userprograms}\{#Name}"; Filename: "{app}\gracil.exe"

[Run]
Filename: "{app}\gracil.exe"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; What Gracil itself put in Windows (ADR 0054): Send to's shortcut. Starting with Windows is a
; RunOnce value below; the main menu, written by an administrator, Gracil takes out itself.
Type: files; Name: "{usersendto}\{#Name}.lnk"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\RunOnce"; ValueType: none; ValueName: "{#Name}"; Flags: uninsdeletevalue dontcreatekey
