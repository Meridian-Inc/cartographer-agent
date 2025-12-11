; Cartographer Agent NSIS Installer Hooks
; Handles Npcap installation as a prerequisite for network scanning
; Compatible with Tauri 2.0 NSIS bundler

!include "LogicLib.nsh"
!include "x64.nsh"

; Custom install macro - called during installation
!macro customInstall
    ; Check if Npcap or WinPcap is already installed
    StrCpy $0 ""
    
    ; Check for Npcap in registry (64-bit)
    ${If} ${RunningX64}
        SetRegView 64
    ${EndIf}
    
    ClearErrors
    ReadRegStr $0 HKLM "SOFTWARE\Npcap" ""
    
    ${If} $0 == ""
        ; Npcap not found, check for WinPcap
        ClearErrors
        ReadRegStr $0 HKLM "SOFTWARE\WinPcap" ""
    ${EndIf}
    
    ; Reset registry view
    ${If} ${RunningX64}
        SetRegView 32
    ${EndIf}
    
    ${If} $0 == ""
        ; Neither Npcap nor WinPcap is installed
        MessageBox MB_YESNO|MB_ICONQUESTION \
            "Cartographer Agent requires Npcap for network scanning.$\r$\n$\r$\n\
Npcap is not currently installed on your system.$\r$\n$\r$\n\
Would you like to install Npcap now?$\r$\n$\r$\n\
(Npcap is free for personal use)" \
            IDYES InstallNpcap IDNO SkipNpcap
        
        InstallNpcap:
            DetailPrint "Installing Npcap..."
            
            ; Extract Npcap installer to temp directory
            SetOutPath $PLUGINSDIR
            ; Path relative to generated script in target/release/nsis/x64/
            File "..\..\..\..\nsis\npcap-installer.exe"
            
            ; Run Npcap installer - user will see the Npcap installer UI
            ExecWait '"$PLUGINSDIR\npcap-installer.exe"' $1
            
            DetailPrint "Npcap installer exited with code: $1"
            
            ${If} $1 != 0
                MessageBox MB_OK|MB_ICONINFORMATION \
                    "Npcap installation may require a restart.$\r$\n$\r$\n\
If network scanning doesn't work, please restart your computer."
            ${EndIf}
            
            ; Clean up
            Delete "$PLUGINSDIR\npcap-installer.exe"
            
            Goto NpcapDone
        
        SkipNpcap:
            MessageBox MB_OK|MB_ICONINFORMATION \
                "Cartographer Agent will be installed without Npcap.$\r$\n$\r$\n\
Network scanning features will not work until Npcap is installed.$\r$\n$\r$\n\
You can download Npcap later from: https://npcap.com"
        
        NpcapDone:
    ${Else}
        DetailPrint "Npcap/WinPcap is already installed"
    ${EndIf}
!macroend

; Custom uninstall macro - called during uninstallation
!macro customUninstall
    ; Nothing special needed during uninstall
    ; We don't uninstall Npcap as other apps might need it
!macroend
