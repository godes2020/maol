; Installer for native Moa (Inno Setup 6). Built in CI:
;   iscc /DAppVersion=0.5.0 /DSrcExe=..\target\release\moa.exe installer\moa.iss
; Installs per user (no admin), closes a running Moa, removes Moa 0.3 and
; earlier builds through their own uninstallers and clears what they left in
; %APPDATA%\Moa. Settings (config.json) and player sign-ins (WebView2) stay.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SrcExe
  #define SrcExe "..\target\release\moa.exe"
#endif

[Setup]
AppId={{6F1C2B9E-7D4A-4E52-9A0B-3C8D5E1F2A47}
AppName=Moa
AppVersion={#AppVersion}
AppVerName=Moa {#AppVersion}
AppPublisher=godes2020
AppPublisherURL=https://github.com/godes2020/maol
DefaultDirName={localappdata}\Programs\Moa
DefaultGroupName=Moa
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Moa-Setup-{#AppVersion}-preview-x64
SetupIconFile=..\assets\moa.ico
UninstallDisplayIcon={app}\Moa.exe
UninstallDisplayName=Moa
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
CloseApplications=no

[Languages]
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
ru.DesktopIcon=Ярлык на рабочем столе
en.DesktopIcon=Desktop shortcut
ru.Removing=Удаляю старую версию Moa…
en.Removing=Removing the old Moa…

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"

[Files]
Source: "{#SrcExe}"; DestDir: "{app}"; DestName: "Moa.exe"; Flags: ignoreversion
Source: "..\..\extension\*"; DestDir: "{app}\extension"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\Moa"; Filename: "{app}\Moa.exe"
Name: "{autodesktop}\Moa"; Filename: "{app}\Moa.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Moa.exe"; Description: "{cm:LaunchProgram,Moa}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /T /IM Moa.exe"; Flags: runhidden; RunOnceId: "KillMoa"

[UninstallDelete]
Type: filesandordirs; Name: "{app}"

[Registry]
; Start with Windows points at the exe that set it; Moa writes it again on start.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Moa"; Flags: deletevalue uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "electron.app.Moa"; Flags: deletevalue

[Code]
const
  UninstallRoot = 'Software\Microsoft\Windows\CurrentVersion\Uninstall';
  OwnKey = '{6F1C2B9E-7D4A-4E52-9A0B-3C8D5E1F2A47}_is1';

procedure KillMoa;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /T /IM Moa.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /T /IM MoaMedia.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Sleep(500);
end;

{ Splits '"C:\x\Uninstall Moa.exe" /S' into the program and its arguments. }
procedure SplitCommand(const Cmd: String; var Prog, Args: String);
var
  S: String;
  P: Integer;
begin
  S := Trim(Cmd);
  if (Length(S) > 0) and (S[1] = '"') then
  begin
    Delete(S, 1, 1);
    P := Pos('"', S);
    if P = 0 then P := Length(S) + 1;
    Prog := Copy(S, 1, P - 1);
    Args := Trim(Copy(S, P + 1, Length(S)));
  end
  else
  begin
    P := Pos(' /', S);
    if P = 0 then P := Length(S) + 1;
    Prog := Copy(S, 1, P - 1);
    Args := Trim(Copy(S, P + 1, Length(S)));
  end;
end;

{ Runs the silent uninstaller of every other "Moa" entry under one root. }
{ NSIS uninstallers copy themselves to %TEMP% and return at once, so wait
  until the original is gone (or give up after 60 s). }
procedure WaitGone(const Path: String);
var
  I: Integer;
begin
  for I := 1 to 120 do
  begin
    if not FileExists(Path) then Exit;
    Sleep(500);
  end;
end;

procedure RemoveOldFrom(Root: Integer);
var
  Keys: TArrayOfString;
  I, Code: Integer;
  Name, Cmd, Prog, Args: String;
begin
  if not RegGetSubkeyNames(Root, UninstallRoot, Keys) then Exit;
  for I := 0 to GetArrayLength(Keys) - 1 do
  begin
    if CompareText(Keys[I], OwnKey) = 0 then Continue;
    if not RegQueryStringValue(Root, UninstallRoot + '\' + Keys[I], 'DisplayName', Name) then Continue;
    if (CompareText(Name, 'Moa') <> 0) and (Pos('Moa ', Name) <> 1) then Continue;
    if not RegQueryStringValue(Root, UninstallRoot + '\' + Keys[I], 'QuietUninstallString', Cmd) then
    begin
      if not RegQueryStringValue(Root, UninstallRoot + '\' + Keys[I], 'UninstallString', Cmd) then Continue;
      Cmd := Cmd + ' /S';
    end;
    SplitCommand(Cmd, Prog, Args);
    if FileExists(Prog) then
    begin
      Log('Removing old Moa: ' + Cmd);
      { ShellExec so an uninstaller that needs admin can ask for it. }
      ShellExec('', Prog, Args, '', SW_HIDE, ewWaitUntilTerminated, Code);
      WaitGone(Prog);
      { Whatever the old uninstaller left behind, if it is a Moa folder. }
      if CompareText(ExtractFileName(ExtractFileDir(Prog)), 'Moa') = 0 then
        DelTree(ExtractFileDir(Prog), True, True, True);
    end;
    RegDeleteKeyIncludingSubkeys(Root, UninstallRoot + '\' + Keys[I]);
  end;
end;

{ Moa 0.3 (Electron) kept caches next to the settings. Only the settings and
  the player's sign-ins are kept. }
procedure CleanAppData;
var
  Dir: String;
  F: TFindRec;
begin
  Dir := ExpandConstant('{userappdata}\Moa');
  if FindFirst(Dir + '\*', F) then
  try
    repeat
      if (F.Name = '.') or (F.Name = '..') then Continue;
      if (CompareText(F.Name, 'config.json') = 0) or (CompareText(F.Name, 'WebView2') = 0) then Continue;
      if F.Attributes and FILE_ATTRIBUTE_DIRECTORY <> 0 then
        DelTree(Dir + '\' + F.Name, True, True, True)
      else
        DeleteFile(Dir + '\' + F.Name);
    until not FindNext(F);
  finally
    FindClose(F);
  end;
  DelTree(ExpandConstant('{localappdata}\moa-updater'), True, True, True);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  WizardForm.PreparingLabel.Caption := CustomMessage('Removing');
  KillMoa;
  RemoveOldFrom(HKCU);
  RemoveOldFrom(HKLM);
  if IsWin64 then RemoveOldFrom(HKLM64);
  KillMoa;
  CleanAppData;
  Result := '';
end;
