; Loner-Strike's additions to Tauri's Windows installer and uninstaller (tauri.conf.json:
; bundle > windows > nsis > installerHooks). The folder clean-up below doesn't run when an app
; update replaces the old version ($UpdateMode = 1): only a real uninstall from Programs and Features.
;
; Only Loner-Strike's own, named folders, never the install folder as a whole: a person
; can install into a folder of their own (D:\Games), and %LOCALAPPDATA%\Loner-Strike also
; holds the dedicated servers (Servers\), which an uninstall keeps.

; The helpers that run beside the app (voice commands, playit's agent) keep running when it
; quits for an update, and a running program can't be replaced: they're stopped first, on
; updates and uninstalls alike. (They start again with the server, or when turned on.)
!macro LS_STOP_HELPERS
  nsExec::Exec 'taskkill /F /T /IM ls-voice.exe'
  Pop $0
  nsExec::Exec 'taskkill /F /T /IM ls-intent.exe'
  Pop $0
  nsExec::Exec 'taskkill /F /T /IM playitd.exe'
  Pop $0
  Sleep 500
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro LS_STOP_HELPERS
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro LS_STOP_HELPERS
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    ; setup's cache from 0.1.3 and earlier (always here, wherever the app was installed)
    RMDir /r "$LOCALAPPDATA\${PRODUCTNAME}\work"
    RMDir /r "$LOCALAPPDATA\${PRODUCTNAME}\downloads"
    ; the folder itself only once it's empty (not while it holds servers)
    RMDir "$LOCALAPPDATA\${PRODUCTNAME}"
  ${EndIf}

  ; "Delete the application data": Tauri removes the folders named after the app's id
  ; (gg.lonerstrike.app). Loner-Strike keeps its settings and records under its own name
  ; (%APPDATA%\Loner-Strike), so that goes too. Unticked, it stays for a reinstall.
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    RMDir /r "$APPDATA\${PRODUCTNAME}"
  ${EndIf}
!macroend
