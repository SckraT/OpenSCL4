#define AppName "OpenSCL4"
#define AppExeName "openscl4.exe"
#ifndef AppVersion
  #error Build with scripts/release.py to supply the product version
#endif
#ifndef SourceDir
  #error Build with scripts/release.py to supply the staging directory
#endif

[Setup]
AppId=io.github.sckrat.openscl4
AppName={#AppName}
AppVersion={#AppVersion}
VersionInfoVersion={#AppVersion}
AppCopyright=Based on OpenSCQ30, (C) Kyle Scheuing and contributors
AppPublisher=SckraT
AppPublisherURL=https://github.com/SckraT/OpenSCL4
OutputBaseFilename=openscl4-{#AppVersion}-windows-x86_64-setup
WizardStyle=modern
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=auto
SetupIconFile=..\..\desktop\resources\com.oppzippy.OpenSCQ30.ico
UninstallDisplayIcon={app}\{#AppExeName}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
english.LaunchOpenSCL4=Launch OpenSCL4
russian.LaunchOpenSCL4=Запустить OpenSCL4

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\openscl4.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"; AppUserModelID: "io.github.sckrat.openscl4"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; AppUserModelID: "io.github.sckrat.openscl4"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchOpenSCL4}"; Flags: nowait postinstall skipifsilent
