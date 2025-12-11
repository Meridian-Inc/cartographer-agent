; Cartographer Agent NSIS Installer Hooks
; Handles Npcap installation as a prerequisite for network scanning

!include "LogicLib.nsh"

Var NpcapInstalled

; Check if Npcap or WinPcap is already installed
!macro _CheckNpcapInstalled
    StrCpy $NpcapInstalled "false"
    
    ; Check for Npcap in registry
    ClearErrors
    ReadRegStr $0 HKLM "SOFTWARE\Npcap" ""
    ${IfNot} ${Errors}
        ${If} $0 != ""
            StrCpy $NpcapInstalled "true"
        ${EndIf}
    ${EndIf}
    
    ; Also check for WinPcap as fallback
    ${If} $NpcapInstalled == "false"
        ClearErrors
        ReadRegStr $0 HKLM "SOFTWARE\WinPcap" ""
        ${IfNot} ${Errors}
            ${If} $0 != ""
                StrCpy $NpcapInstalled "true"
            ${EndIf}
        ${EndIf}
    ${EndIf}
!macroend

; Called before the install section
!macro NSIS_HOOK_PREINSTALL
    !insertmacro _CheckNpcapInstalled
    
    ${If} $NpcapInstalled == "false"
        MessageBox MB_YESNO|MB_ICONQUESTION \
            "Cartographer Agent requires Npcap for network scanning.$\r$\n$\r$\n\
            Npcap is not currently installed on your system.$\r$\n$\r$\n\
            Would you like to install Npcap now?$\r$\n$\r$\n\
            (Npcap is free for personal use)" \
            IDYES npcap_install IDNO npcap_skip
        
        npcap_install:
            DetailPrint "Extracting Npcap installer..."
            
            ; Extract Npcap installer to temp directory
            SetOutPath $PLUGINSDIR
            File "npcap-installer.exe"
            
            DetailPrint "Installing Npcap..."
            
            ; Run Npcap installer with WinPcap API compatibility mode
            ExecWait '"$PLUGINSDIR\npcap-installer.exe" /winpcap_mode=yes' $0
            
            ${If} $0 == "0"
                DetailPrint "Npcap installed successfully"
            ${Else}
                DetailPrint "Npcap installation completed with code: $0"
                MessageBox MB_OK|MB_ICONINFORMATION \
                    "Npcap installation may require a system restart.$\r$\n$\r$\n\
                    If network scanning doesn't work after installation,$\r$\n\
                    please restart your computer."
            ${EndIf}
            
            ; Clean up
            Delete "$PLUGINSDIR\npcap-installer.exe"
            
            Goto npcap_done
        
        npcap_skip:
            MessageBox MB_OK|MB_ICONINFORMATION \
                "Cartographer Agent will be installed without Npcap.$\r$\n$\r$\n\
                Network scanning features will not work until Npcap is installed.$\r$\n$\r$\n\
                You can download Npcap later from: https://npcap.com"
        
        npcap_done:
    ${Else}
        DetailPrint "Npcap/WinPcap is already installed"
    ${EndIf}
!macroend

; Called after the install section
!macro NSIS_HOOK_POSTINSTALL
    ; Installation complete - nothing special needed
!macroend
